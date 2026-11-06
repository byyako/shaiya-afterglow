#!/usr/bin/env python3
"""Part 2 of the balance analysis (docs/plans/balance/2-content.md): the inventory of what exists. Builds CSV tables into
data/balance/ (git-ignored) from the definitions (data.py, read-only) and the model (model.py):

    items.csv          every weapon, armour piece, shield, accessory and cloak: limits, stat budget, crafted stats, enchanting, price,
                       drop grade, origins, and its worth against the items of its kind and level
    item_brackets.csv  per kind of item and level: how many, and the median, lowest and highest of its main number
    item_pairs.csv     each Light item next to its Fury counterpart, and what differs
    lapis.csv          every lapis: stats, grade, where it fits, link rate and price, its skill, origins
    tools.csv          Lapisia, hammers, charms, runes, enchant materials and boxes: what they do and where they come from
    enchant.csv        each enchant step: rates, expected tries and expected cost in money and items
    skills.csv         every learnable skill row: who, level, mode, cost, timing, damage, effect, built or not, the model's damage
    mobs.csv           every monster: numbers, attack per second, experience per HP, placement, the model's fight against the
                       class defaults at its level
    origins.csv        every origin of every item (one row per way), with the chance
    client_items.csv, client_skills.csv   where the client's copy differs from defs, for items and skills a player can get or learn

    python3 content.py [--refresh] [--only items,skills,...] [--quick]

`--quick` leaves the model's numbers out (the skills' and monsters' expected damage, a few minutes otherwise). Everything is read-only.
"""
from __future__ import annotations

import argparse
import csv
import statistics
from dataclasses import replace
from functools import lru_cache

import data
import expect as ex
import formulas as fm
import model

OUT = data.ROOT / "data" / "balance"

REAL_NAMES = {1: "one-handed sword", 2: "two-handed sword", 3: "one-handed axe", 4: "two-handed axe", 5: "double sword", 6: "spear",
              7: "one-handed mace", 8: "two-handed hammer", 9: "short blade", 10: "short sword", 11: "javelin", 12: "staff", 13: "bow",
              14: "crossbow", 15: "claw", 16: "helmet", 17: "upper armour", 18: "lower armour", 19: "shield", 20: "gloves", 21: "boots",
              22: "ring", 23: "amulet", 24: "cloak", 25: "bracelet"}
WEAPONS = range(1, 16)
ARMOUR = range(16, 22)
ACCESSORIES = (22, 23, 25)
JOB_KEYS = "FDRAMP"  # fighter/warrior, defender/guardian, ranger/assassin, archer/hunter, mage/pagan, priest/oracle
MODES = model.MODES
STATS = ("str", "dex", "rec", "int", "wis", "luc")
# The weapon real types a faction's items use in place of the other's (Light one-handed sword for the Fury axe, …); every other kind
# pairs with itself. Read from defs.items: real types 1, 2 and 14 are Light only, 3, 4 and 11 Fury only (country 2 against 5).
MIRROR_REAL = {1: 3, 3: 1, 2: 4, 4: 2, 14: 11, 11: 14}


def _i(v) -> int:
    return int(v) if v not in (None, "") else 0


def write(name: str, rows: list, fields: list | None = None) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    fields = fields or (list(rows[0].keys()) if rows else [])
    with (OUT / name).open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=fields, extrasaction="ignore")
        w.writeheader()
        w.writerows(rows)
    print(f"{name}: {len(rows)} rows")


@lru_cache(maxsize=None)
def raw_items() -> dict:
    """`defs.items` rows by (type, type id), as read (data.rows: the columns the loaders use)."""
    return {(_i(r["type"]), _i(r["typeid"])): r for r in data.rows("items")}


def named(name: str) -> bool:
    """Not a placeholder: `???…`, `...`, `??`, `…` or empty."""
    return bool(name.strip(" ?.…\t")) and not name.startswith(("???", "..."))


def item_name(key) -> tuple[str, str]:
    """The client's name when it has one (and it is not a placeholder), else the defs name; and which it is."""
    c = data.item_names().get(key, "")
    if named(c):
        return c.strip(), "client"
    d = raw_items()[key]["itemname"].strip() if key in raw_items() else ""
    return d, "defs" if d else "none"


def classes_of(jobs, country: int) -> list:
    """The classes that may use an item or learn a skill: its job flags and its race rule (`byCountry`), over the class and race pairs
    that exist (`creation::RACE_CLASSES`)."""
    out = []
    for family, jobs_of_race in enumerate(fm.RACE_CLASSES):
        for job in jobs_of_race:
            if jobs[job] and model.race_fits(country, family):
                out.append(model.class_name(job, family))
    return out


def jobs_letters(jobs) -> str:
    return "".join(k if j else "-" for k, j in zip(JOB_KEYS, jobs))


def craft_numbers(real: int, server: int) -> int:
    """How many of the nine crafted numbers a dropped item gets (`CreateCraftExpansion`, `docs/original/item-origins.md` section 3):
    weapons and accessories the six stats only (1, 2 to 4 that many, 5, 6 or more all six); other items (armour, the cloak) the nine (1, 2
    to 7, 8, 9; above 9 none)."""
    if server <= 0:
        return 0
    if real in WEAPONS or real in ACCESSORIES:
        return min(server, 6)
    return server if server <= 9 else 0


def min_of_draws_mean(top: int, draws: int) -> float:
    """The mean of the smallest of `draws` draws of 0 to `top`: sum over k of P(all draws ≥ k)."""
    n = top + 1
    return sum(((n - k) / n) ** draws for k in range(1, n))


def crafted_budget(real: int, server: int, top: int, draws: int = 3) -> tuple[float, float]:
    """Expected and highest crafted value in stat points (a point of STR … LUC = 1; a crafted HP, MP or SP point is 100, i.e. 20 stat
    points' worth of REC, DEX or WIS health): weapons and accessories pick among six stats, armour among nine numbers."""
    n = craft_numbers(real, server)
    if n == 0 or top <= 0:
        return 0.0, 0.0
    each = min_of_draws_mean(top, draws)
    if real in WEAPONS or real in ACCESSORIES:
        return n * each, n * top
    # Each of the nine is chosen with chance n / 9; six are worth 1 a point, three 20.
    return n / 9 * (6 + 3 * 20) * each, top * (min(n, 3) * 20 + max(n - 3, 0))


def health_points(hp: int, mp: int, sp: int) -> float:
    """HP, MP and SP in stat points: 5 of each is what one REC, WIS or DEX gives."""
    return (hp + mp + sp) / 5.0


def mean_attack(e1: int, e2: int) -> float:
    """The weapon's mean attack: `attack + rand() % spread`."""
    return e1 + (max(e2, 1) - 1) / 2.0


# --- items ---------------------------------------------------------------------------------------------------------------------------


def one_in(chance: float) -> str:
    """`1 in N` for a chance below 1; an item several entries give more than once a kill on average: `N a kill`."""
    return f"1 in {1 / chance:,.0f}" if chance <= 1 else f"{chance:.2g} a kill"


def origin_summary(key) -> dict:
    found = data.origins().get(key, [])
    kinds = sorted({o["kind"] for o in found})
    drops = [o for o in found if o["kind"] == "drop"]
    best = max(drops, key=lambda o: o["chance"], default=None)
    sides = sorted({o["side"] for o in found if o["kind"] == "shop"})
    quests = [o for o in found if o["kind"] == "quest"]
    return {
        "origins": " ".join(kinds),
        "obtainable": bool(set(kinds) - model.RARE - {"mall", "mall box"}),
        "shop_sides": " ".join(sides),
        "drop_mobs": len(drops),
        "best_drop": f"{best['detail']} {one_in(best['chance'])}" if best else "",
        "lowest_drop_level": min((o["level"] for o in drops), default=""),
        "quests": len(quests),
        "lowest_quest_level": min((o["level"] for o in quests), default=""),
    }


def item_rows() -> list:
    defs = data.items()
    rows = []
    for key, r in sorted(raw_items().items()):
        t, tid = key
        real = fm.real_type(t)
        if real == 0 and t != 30:
            continue
        if t == 30:
            continue  # lapis: lapis.csv
        d = defs.get(t, tid)
        name, name_from = item_name(key)
        who = classes_of(d.jobs, d.country)
        e1, e2, e3, e4 = d.effects
        hp, mp, sp = d.const_health
        server, top = _i(r["server"]), max(_i(r["reqwis"]), 0)
        craft_exp, craft_max = crafted_budget(real, server, top) if d.craftable else (0.0, 0.0)
        material = _i(r["reqluc"]) & 0xFFFF
        mat_key = (material // 1000, material % 1000) if material else None
        enchantable = d.craftable and material != 0 and mat_key in raw_items() and real != 24
        budget = sum(d.const_stats) + health_points(hp, mp, sp)
        interval = fm.attack_interval_ms(fm.clamp(d.attack_time, 0, fm.MAX_ATTACK_SPEED))
        row = {
            "type": t, "typeid": tid, "itemid": _i(r["itemid"]), "name": name, "name_from": name_from,
            "defs_name": r["itemname"].strip(), "kind": REAL_NAMES.get(real, ""), "real_type": real,
            "level": d.level, "grow": d.grow, "lowest_mode": MODES[min(d.grow, 3)], "jobs": jobs_letters(d.jobs), "country": d.country,
            "side": data.side_of_country(d.country), "classes": " ".join(who), "usable": bool(who) and d.level <= 60,
            "attack": e1, "spread": e2, "mean_attack": mean_attack(e1, e2) if real in WEAPONS else "",
            "attack_speed": d.attack_time, "interval_ms": interval if real in WEAPONS else "",
            "attack_per_s": round(mean_attack(e1, e2) * 1000 / interval, 1) if real in WEAPONS else "",
            "defence": e3, "magic_defence": e4,
            **dict(zip(STATS, d.const_stats)), "hp": hp, "mp": mp, "sp": sp,
            "move_speed": d.speed, "range": d.range, "absorption": d.absorption, "element": d.attrib, "special": d.special,
            "stat_budget": round(budget, 1),
            "craftable": d.craftable, "craft_numbers": craft_numbers(real, server) if d.craftable else 0, "craft_max": top if d.craftable else 0,
            "craft_expected": round(craft_exp, 1), "craft_highest": round(craft_max, 1),
            "enchantable": enchantable, "enchant_material": f"{mat_key[0]}/{mat_key[1]}" if enchantable else "",
            "enchant_top": (12 * 7 if real in WEAPONS else 13 * 5) if enchantable else "",
            "lapis_room": d.room, "quality": d.quality, "buy": d.buy, "sell": _i(r["sell"]), "grade": d.grade, "drop_limit": _i(r["drop"]),
            "trade_locked": _i(r["reqog"]) == 1,
            **origin_summary(key),
        }
        rows.append(row)
    rate_items(rows)
    return rows


def main_value(r: dict) -> float:
    """The number an item is compared by within its kind: mean attack (weapons); defence + magic defence + stat points (armour, the
    shield: their HP, MP and SP are most of what they give); stat points + the four effects (accessories, cloaks)."""
    real = r["real_type"]
    if real in WEAPONS:
        return float(r["mean_attack"])
    if real in ARMOUR or real == 19:
        return float(r["defence"]) + float(r["magic_defence"]) + float(r["stat_budget"])
    return float(r["stat_budget"]) + float(r["attack"]) + float(r["defence"]) + float(r["magic_defence"])


def rate_items(rows: list) -> None:
    """Each named, usable item against its peers: the same kind within 3 levels (at least 4 of them). `ratio` = its main number over the
    peers' median; an outlier at 1.5 and above or 0.67 and below. Also the main number per level."""
    pool = [r for r in rows if r["usable"] and named(r["name"]) and r["level"] > 0]
    by_kind: dict = {}
    for r in pool:
        by_kind.setdefault(r["real_type"], []).append(r)
    for r in rows:
        r.update({"value": round(main_value(r), 1), "value_per_level": round(main_value(r) / r["level"], 2) if r["level"] else "",
                  "peers": 0, "peer_median": "", "ratio": "", "outlier": ""})
    for real, items in by_kind.items():
        for r in items:
            peers = [p for p in items if abs(p["level"] - r["level"]) <= 3]
            values = [main_value(p) for p in peers]
            med = statistics.median(values)
            r["peers"], r["peer_median"] = len(peers), round(med, 1)
            if len(peers) >= 4 and med > 0:
                ratio = main_value(r) / med
                r["ratio"] = round(ratio, 2)
                r["outlier"] = "high" if ratio >= 1.5 else "low" if ratio <= 0.67 else ""


def bracket_rows(items: list) -> list:
    out = []
    groups: dict = {}
    for r in items:
        if r["usable"] and named(r["name"]) and r["level"] > 0:
            groups.setdefault((r["real_type"], r["level"]), []).append(r)
    for (real, level), rs in sorted(groups.items()):
        values = [main_value(r) for r in rs]
        budgets = [r["stat_budget"] for r in rs]
        out.append({"kind": REAL_NAMES[real], "real_type": real, "level": level, "count": len(rs),
                    "obtainable": sum(r["obtainable"] for r in rs),
                    "main": "mean attack" if real in WEAPONS else "defence + magic defence + stat points" if real in ARMOUR or real == 19 else "stat points + effects",
                    "median": round(statistics.median(values), 1), "lowest": min(values), "highest": max(values),
                    "median_per_level": round(statistics.median(values) / level, 2),
                    "median_stat_points": round(statistics.median(budgets), 1),
                    "median_magic_defence": statistics.median(r["magic_defence"] for r in rs)})
    return out


PAIR_FIELDS = ("attack", "spread", "attack_speed", "defence", "magic_defence") + STATS + ("hp", "mp", "sp", "move_speed", "range",
               "absorption", "element", "craft_numbers", "craft_max", "lapis_room", "quality", "buy", "enchantable")


def pair_rows(items: list) -> list:
    """Each Light-only item (country 0 to 2) with a Fury-only item (3 to 5) of the mirrored kind (`MIRROR_REAL`), the same job flags,
    level and mode. When a group has several on a side, the closest pairs first (fewest differing numbers, then the type ids). What
    differs."""
    def key(r, mirror):
        real = MIRROR_REAL.get(r["real_type"], r["real_type"]) if mirror else r["real_type"]
        # The archer's flag differs by side on one-handed swords and axes and on double swords (Light swords and Fury double swords
        # allow it): those pair without it, and the job flags count as a difference.
        jobs = r["jobs"][:3] + "-" + r["jobs"][4:] if r["real_type"] in (1, 3, 5) else r["jobs"]
        return (real, jobs, r["level"], r["grow"])

    def diffs(a, b):
        return [f"{f} {a[f]}/{b[f]}" for f in ("jobs",) + PAIR_FIELDS if a[f] != b[f]]

    pool = [r for r in items if named(r["name"]) and r["usable"]]
    light, fury = {}, {}
    for r in pool:
        if r["side"] == "light":
            light.setdefault(key(r, True), []).append(r)
        elif r["side"] == "fury":
            fury.setdefault(key(r, False), []).append(r)
    out = []
    for k in sorted(set(light) | set(fury), key=str):
        ls, fs = list(light.get(k, [])), list(fury.get(k, []))
        pairs = []
        options = sorted(((len(diffs(a, b)), a["type"], a["typeid"], b["type"], b["typeid"], i, j)
                          for i, a in enumerate(ls) for j, b in enumerate(fs)))
        used_l, used_f = set(), set()
        for *_, i, j in options:
            if i not in used_l and j not in used_f:
                used_l.add(i)
                used_f.add(j)
                pairs.append((ls[i], fs[j]))
        pairs += [(a, None) for i, a in enumerate(ls) if i not in used_l] + [(None, b) for j, b in enumerate(fs) if j not in used_f]
        for a, b in pairs:
            d = diffs(a, b) if a and b else []
            x = a or b
            out.append({
                "kind": REAL_NAMES[x["real_type"]] + (f" / {REAL_NAMES[b['real_type']]}" if a and b and a["real_type"] != b["real_type"] else ""),
                "jobs": k[1], "level": k[2], "grow": k[3],
                "light": f"{a['type']}/{a['typeid']} {a['name']}" if a else "", "fury": f"{b['type']}/{b['typeid']} {b['name']}" if b else "",
                "light_obtainable": a["obtainable"] if a else "", "fury_obtainable": b["obtainable"] if b else "",
                "status": "no Fury counterpart" if not b else "no Light counterpart" if not a else "differs" if d else "mirrored",
                "differences": "; ".join(d),
                "light_value": main_value(a) if a else "", "fury_value": main_value(b) if b else "",
                "light_stat_points": a["stat_budget"] if a else "", "fury_stat_points": b["stat_budget"] if b else "",
            })
    return out


def duplicate_rows(items: list) -> list:
    """Named items whose every number is the same as another's (only the key and name differ)."""
    fields = ("real_type", "level", "grow", "jobs", "country") + PAIR_FIELDS + ("special", "grade")
    groups: dict = {}
    for r in items:
        if named(r["name"]):
            groups.setdefault(tuple(r[f] for f in fields), []).append(r)
    return [g for g in groups.values() if len(g) > 1]


# --- lapis and tools -----------------------------------------------------------------------------------------------------------------

LAPIS_FITS = (("weapon", "reqlevel"), ("helmet", "country"), ("upper", "attackfighter"), ("lower", "defensefighter"),
              ("shield", "patrolrogue"), ("gloves", "shootrogue"), ("boots", "attackmage"), ("ring", "defensemage"), ("amulet", "grow"),
              ("bracelet", "reqstr"))


def gem_price(grade: int) -> int:
    """`lapis::gem_price` (`GetGemMoneyByGrade`)."""
    low = [1_000, 4_095, 11_250, 22_965, 41_280, 137_900, 365_000, 480_000, 627_000, 814_000, 1_040_000]
    high = [2_500_000, 3_500_000, 5_500_000, 7_500_000, 9_800_000]
    if 31 <= grade <= 40:
        return low[grade - 30]
    if 96 <= grade <= 100:
        return high[grade - 96]
    return 0


def base_rate(grade: int, level: int, kc: int = 0) -> int:
    """`lapis::base_rate`: `trunc(f32(289 − 8g) + f32(KC) + f32(level) × 0.05f)`, at least 1."""
    fixed = fm.f32(289 - 8 * grade) + float(kc)
    return max(int(fixed + fm.f32(level) * fm.f32(0.05)), 1)


def link_rate(grade: int, level: int = 60, hammer: int = 0, premium: bool = False, kc: int = 0) -> int:
    """`lapis::link`'s rate in per cent: the base rate, × a hammer's `reqvg` %, × 1.5 premium, clamped 1 to 50 (100 for a hammer of
    1000 % or more)."""
    rate = base_rate(grade, level, kc)
    if hammer:
        rate = int(fm.f32(hammer) / 100.0 * fm.f32(rate))
    if premium:
        rate = int(fm.f32(rate * 1.5))
    if hammer >= 1000:
        return 100
    return fm.clamp(rate, 1, 50)


def lapis_rows() -> list:
    sk = data.skills()
    out = []
    for key, r in sorted(raw_items().items()):
        if key[0] != 30:
            continue
        name, name_from = item_name(key)
        grade = _i(r["reqig"])
        fits = [slot for slot, col in LAPIS_FITS if _i(r[col]) != 0]
        special = _i(r["special"])
        # A special that names a level-100 skill is the lapis' option skill (`docs/original/item-options.md` section 1): the weapon's
        # strike the target, an armour's the attacker; 1 to 15 outside the weapon also block that state.
        skill = sk.get((special, 100)) if special and special != 16 else None
        out.append({
            "typeid": key[1], "name": name, "name_from": name_from, "defs_name": r["itemname"].strip(), "grade": grade,
            "fits": " ".join(fits), "room": _i(r["slot"]), "dangerous": _i(r["reqvg"]) != 0,
            "attack": _i(r["effect1"]), "spread": _i(r["effect2"]), "defence": _i(r["effect3"]), "magic_defence": _i(r["effect4"]),
            **{s: _i(r["const" + s]) for s in STATS}, "hp": _i(r["consthp"]), "mp": _i(r["constmp"]), "sp": _i(r["constsp"]),
            "attack_speed": _i(r["attacktime"]), "move_speed": _i(r["speed"]), "range": _i(r["range"]), "absorption": _i(r["exp"]),
            "element": _i(r["attrib"]), "special": special,
            "state_block": special if 1 <= special < 16 else "",
            "option_skill": f"{special} {skill.name}" if skill else "",
            "option_what": (f"{KINDS.get((skill.type_effect, skill.kind), skill.kind)}"
                            + (f", {DAMAGE_TYPES.get(skill.damage_type)} {skill.damage[0]}" if skill.type_effect == fm.ATTACK else "")
                            + (f", success type {skill.success_type} {skill.success_value}" if skill.success_type else ", magic resist roll")
                            + (f", {skill.keep_time} s" if skill.keep_time and skill.type_effect == fm.DEBUFF else "")) if skill else "",
            "stat_points": round(sum(_i(r["const" + s]) for s in STATS) + health_points(_i(r["consthp"]), _i(r["constmp"]), _i(r["constsp"])), 1),
            "quality": _i(r["quality"]), "price": gem_price(grade), "link_60": link_rate(grade), "link_60_premium": link_rate(grade, premium=True),
            "link_60_hammer_450": link_rate(grade, hammer=450), "link_30": link_rate(grade, 30), "link_15": link_rate(grade, 15),
            "buy": _i(r["buy"]), "drop_grade": _i(r["grade"]),
            **origin_summary(key),
        })
    return out


TOOL_SPECIALS = {36: "Linking Hammer", 57: "Lucky Charm", 58: "Extraction Hammer", 62: "Recreation Rune", 42: "box"}


def tool_rows(items: list) -> list:
    materials = {r["enchant_material"] for r in items if r["enchant_material"]}
    users: dict = {}
    for r in items:
        if r["enchant_material"]:
            users[r["enchant_material"]] = users.get(r["enchant_material"], 0) + 1
    out = []
    for key, r in sorted(raw_items().items()):
        special = _i(r["special"])
        mat = f"{key[0]}/{key[1]}"
        if not (key[0] == 95 or special in TOOL_SPECIALS or mat in materials):
            continue
        name, name_from = item_name(key)
        what = ("Lapisia" if key[0] == 95 else TOOL_SPECIALS.get(special, "") if special in TOOL_SPECIALS else "")
        if mat in materials:
            what = (what + " and " if what else "") + f"enchant material of {users[mat]} items"
        fits = [slot for slot, col in LAPIS_FITS if _i(r[col]) != 0] if key[0] == 95 else []
        out.append({"type": key[0], "typeid": key[1], "name": name, "name_from": name_from, "what": what,
                    "grade": _i(r["reqig"]), "reqvg": _i(r["reqvg"]), "fits": " ".join(fits),
                    "price_per_use": gem_price(_i(r["reqig"])) if key[0] == 95 else "",
                    "buy": _i(r["buy"]), "drop_grade": _i(r["grade"]), "box_set": _i(r["reqvg"]) if special == 42 else "",
                    **origin_summary(key)})
    return out


def big(x: float):
    """A whole number, or three figures beyond a trillion."""
    return round(x) if x < 1e12 else f"{x:.3g}"


def enchant_rows() -> list:
    """Each step from step 0, with one material a try: success when material + a draw below 1,000,000 < rate[step]
    (`Enchant::succeeds`), so the chance is (rate − 1) / 1,000,000. A try uses one Lapisia (its grade price in money: weapon grade 96,
    armour 40) and one material.

    - `safe`: a Lapisia that keeps the item on a failure (95/2, 95/7; no origin in the data): tries add up step by step.
    - `dangerous`: the Lapisia a player can get (95/1, 95/6, `reqvg` 1): a failure destroys the item, so a step is reached only by
      succeeding at every step from 0 with one item; tries `T(n) = (T(n − 1) + 1) / p(n − 1)`, items `1 / (p(0) × … × p(n − 1))`."""
    out = []
    table = data.enchant()  # the imported ItemEnchant.ini, or the built-in tables
    safe = {0: 0.0, 1: 0.0}
    chain = {0: (0.0, 1.0), 1: (0.0, 1.0)}  # expected tries, chance of the whole chain
    for step in range(20):
        row = {"from_step": step, "to_step": step + 1}
        for side, (name, price) in enumerate((("weapon", gem_price(96)), ("armour", gem_price(40)))):
            rate = table.rate(step, bool(side))
            p = max(rate - 1, 0) / 1_000_000
            tries, whole = chain[side]
            if p > 0:
                safe[side] += 1 / p
                chain[side] = ((tries + 1) / p, whole * p)
            else:
                safe[side], chain[side] = float("inf"), (float("inf"), 0.0)
            st, (dt, dp) = safe[side], chain[side]
            row.update({
                f"{name}_bonus": table.bonus(step + 1, bool(side)), f"{name}_rate_millionths": rate, f"{name}_chance": f"{p:.4%}",
                f"{name}_safe_tries": big(st) if st != float("inf") else "never",
                f"{name}_safe_money": big(st * price) if st != float("inf") else "never",
                f"{name}_dangerous_tries": big(dt) if dp else "never",
                f"{name}_dangerous_items": big(1 / dp) if dp else "never",
                f"{name}_dangerous_money": big(dt * price) if dp else "never",
            })
        out.append(row)
    return out


USABLE_TYPES = (25, 44, 100)  # real type 26: items used from the bags (`inventory::real_type`)


def item_skill_rows() -> list:
    """Items used from the bags that apply a skill (`item_use::effect`: `range` and `attacktime` not 0 give the skill id and level; special
    50, the experience wheels, also reset experience): the skill's effect and where the item comes from."""
    sk = data.skills()
    out = []
    for key, r in sorted(raw_items().items()):
        if key[0] not in USABLE_TYPES or not (_i(r["range"]) and _i(r["attacktime"])):
            continue
        sid, lv = _i(r["range"]) & 0xFFFF, _i(r["attacktime"])
        s = sk.get((sid, lv))
        name, name_from = item_name(key)
        out.append({"type": key[0], "typeid": key[1], "name": name, "name_from": name_from, "level": _i(r["reqlevel"]),
                    "special": _i(r["special"]), "skill": f"{sid}/{lv}", "skill_name": s.name if s else "no such skill",
                    "what": KINDS.get((s.type_effect, s.kind), f"wType {s.kind}") if s else "",
                    "abilities": ability_text(s) if s else "", "keep_s": s.keep_time if s else "",
                    "heal": "/".join(map(str, s.heal)) if s and any(s.heal) else "", "buy": _i(r["buy"]),
                    **origin_summary(key)})
    return out


# --- skills --------------------------------------------------------------------------------------------------------------------------

EFFECTS = {0: "passive", 1: "attack", 2: "heal", 3: "debuff", 4: "buff", 5: "field", 6: "one-off"}
KINDS = {  # (type effect, wType) -> what the server does with it (docs/analysis-skills*.md)
    (0, 1): "weapon mastery (attack for listed weapons)", (0, 2): "bags", (0, 4): "abilities", (0, 5): "language (nothing reads it)",
    (0, 6): "abilities while HP is low", (0, 8): "weapon speed", (0, 9): "cross road", (0, 10): "shield defence",
    (1, 21): "attack", (1, 22): "multi-hit", (1, 23): "desperate attack (kills its user)", (1, 24): "attack ignoring the target's element",
    (1, 25): "attack", (1, 27): "drain", (2, 41): "heal", (2, 42): "dispel", (2, 43): "sacrifice", (3, 21): "attack (debuff kind)",
    (3, 61): "damage over time", (3, 62): "lowers abilities", (3, 63): "death at the end", (3, 64): "stun", (3, 65): "stop",
    (3, 66): "sleep", (3, 67): "lowers abilities", (3, 68): "no element", (3, 72): "taunt", (3, 90): "transformation (polymorph)",
    (6, 122): "not built", (4, 69): "taunt (buff)", (4, 81): "heal over time", (4, 82): "abilities", (4, 83): "reflection",
    (4, 84): "Fire Thorns", (4, 85): "blocks (count)", (4, 86): "blocks (chance)", (4, 87): "give up SP/MP for HP", (4, 88): "hiding",
    (4, 90): "transformation", (4, 91): "abilities", (4, 92): "blocks (all)", (4, 93): "detection", (4, 94): "attack element",
    (4, 95): "defence element", (4, 123): "scouting", (5, 101): "trap", (5, 102): "fire wall", (6, 93): "detection",
    (6, 121): "resurrection", (6, 123): "scouting", (6, 125): "return",
}
ABILITIES = {1: "max HP", 2: "max MP", 3: "max SP", 4: "STR", 5: "REC", 6: "INT", 7: "WIS", 8: "DEX", 9: "LUC", 10: "sit HP %",
             11: "sit SP %", 12: "sit MP %", 13: "SP cost %", 14: "MP cost %", 15: "range", 16: "attack speed", 17: "move speed",
             18: "critical", 19: "cooldown cut", 20: "melee hit", 21: "ranged hit", 22: "magic hit", 23: "melee attack",
             24: "ranged attack", 25: "magic attack", 26: "melee defence", 27: "ranged defence", 28: "magic defence",
             29: "melee evasion", 30: "ranged evasion", 31: "magic evasion", 32: "no melee", 33: "no ranged", 34: "no magic"}
DAMAGE_TYPES = {0: "fixed", 1: "attack + value", 2: "formula × value", 3: "value × caster's current", 4: "value % of target max",
                5: "value % of target current"}
ATTACK_KINDS = ("melee", "ranged", "magic")


def not_built(s: fm.SkillDef) -> bool:
    """`SkillDef::not_built` (skills.rs): fields other than traps and fire walls, one-off skills other than resurrection, scouting and
    return, and the not-seen buffs (89)."""
    if s.type_effect == fm.FIELD:
        return s.kind not in (101, 102)
    if s.type_effect == fm.ONCE:
        return s.kind not in (121, 123, 125)
    return s.type_effect == fm.BUFF and s.kind == 89


def ability_text(s: fm.SkillDef) -> str:
    return ", ".join(f"{ABILITIES.get(t, f'ability {t}')} {'+' if s.type_effect != fm.DEBUFF else '-'}{v}" for t, v in s.abilities if t)


# Part 1's reference monsters (scenarios.REFERENCE): the most placed monster with experience at levels 15, 30 and, for 60, 1547 (level
# 58, the most placed of the level-57/58 monsters of maps 45 to 47; the level-59 and 60 ones are party content).
REFERENCE = {15: 2125, 30: 2183, 60: 1547}
# Other levels: the most placed monster with experience and at most this much HP (leaves out the party and boss monsters).
SOLO_HP = 30_000


@lru_cache(maxsize=None)
def reference_mob(level: int) -> fm.MobDef:
    """Part 1's monster at 15, 30 and 60; otherwise the most placed monster (spawn areas) with experience and at most `SOLO_HP` HP at
    this level, else at the nearest level below, then above."""
    mobs = data.mobs()
    if level in REFERENCE:
        return mobs[REFERENCE[level]]
    counts: dict = {}
    for mob, places in data.placed().items():
        d = mobs.get(mob)
        if d and d.exp > 0 and d.hp <= SOLO_HP:
            counts[mob] = counts.get(mob, 0) + sum(c for _, c, h in places if h == "area")
    for delta in [0] + [x for k in range(1, 10) for x in (-k, k)]:
        at = [(c, m) for m, c in counts.items() if mobs[m].level == level + delta and c > 0]
        if at:
            return mobs[max(at)[1]]
    raise SystemExit(f"no monster near level {level}")


@lru_cache(maxsize=None)
def build(job: int, side: str, level: int, weapon: int | None = None, shield: bool = False) -> model.Character | None:
    name = (model.LIGHT if side == "light" else model.FURY)[job]
    spec = f"{name} ultimate {level}" + (f" weapon={weapon}" if weapon else "") + (" shield=1" if shield else "")
    try:
        return model.evaluate(model.parse_build(spec))
    except (SystemExit, KeyError):
        return None


def weapon_for(s: fm.SkillDef, job: int, side: str, level: int):
    """A weapon real type the skill allows that the class can wear at this level (the default build's first, else the first that
    fits), or the shield."""
    c = build(job, side, level)
    if c is None:
        return None
    if s.need_shield:
        return ("shield",)
    allowed = [w + 1 for w, ok in enumerate(s.need_weapons) if ok]
    if not allowed:
        return None
    current = next((fm.real_type(it.item_type) for it in c.build.items if it.slot == 5), 0)
    if current in allowed:
        return (None,)
    for w in allowed:
        alt = build(job, side, level, w)
        if alt and any(fm.real_type(it.item_type) == w for it in alt.build.items if it.slot == 5):
            return (w,)
    return None


def skill_numbers(s: fm.SkillDef, job: int, level: int) -> dict:
    """The model's expected damage of one use against the reference monster of the level, and against the mirrored class (Fury,
    default build) in PvP, with the Light class's default build at this level and a weapon the skill allows."""
    w = weapon_for(s, job, "light", level)
    if w is None:
        return {"model_note": "no weapon of the class fits"}
    c = build(job, "light", level, None if w[0] in (None, "shield") else w[0], w[0] == "shield")
    t = build(job, "fury", level)
    if c is None or t is None:
        return {"model_note": "no build"}
    c = replace(c, attack_skills=[s])
    d = reference_mob(level)
    acts = model.actions_vs_mob(c, d)
    normal = next((a for a in acts if a.skill is None), None)
    mine = next((a for a in acts if a.skill is not None), None)
    pvp = model.actions_vs_player(c, t)
    pvp_mine = next((a for a in pvp if a.skill is not None), None)
    pvp_normal = next((a for a in pvp if a.skill is None), None)
    out = {"model_mob": f"{d.id} (level {d.level})"}
    if mine:
        dot = mine.tick * mine.ticks
        out.update({"pve_damage": round(mine.damage + dot), "pve_per_s": round((mine.damage + dot) / mine.time_ms * 1000),
                    "pve_normal_per_s": round(normal.damage / normal.time_ms * 1000) if normal else "",
                    "pve_share_of_mob_hp": round((mine.damage + dot) / d.hp, 3)})
    if pvp_mine:
        dot = pvp_mine.tick * pvp_mine.ticks
        out.update({"pvp_damage": round(pvp_mine.damage + dot), "pvp_per_s": round((pvp_mine.damage + dot) / pvp_mine.time_ms * 1000),
                    "pvp_normal_per_s": round(pvp_normal.damage / pvp_normal.time_ms * 1000) if pvp_normal else "",
                    "pvp_share_of_target_hp": round((pvp_mine.damage + dot) / t.max_hp, 3)})
    return out


def class_skill_rows(level: int = 60) -> list:
    """Every attack skill each class (both sides) learns by this level, at its highest level, with the class's default build (a weapon
    the skill allows): expected damage of one use and per second of the attack timer against the reference monster and against the
    other side's mirrored class; whether it is ever better than the normal attack, and whether another of the class's skills is at
    least as good in every way (damage per use, cast and interval, cooldown, SP and MP, the same weapon)."""
    out = []
    sk = data.skills()
    d = reference_mob(level)
    for side, other in (("light", "fury"), ("fury", "light")):
        for job in range(6):
            base = build(job, side, level)
            target = build(job, other, level)
            if base is None or target is None:
                continue
            rows = []
            for sid, lv in base.build.learnt:
                s = sk[(sid, lv)]
                if s.type_effect not in (fm.ATTACK, fm.DEBUFF) or s.target_type not in (3, 6, 7) or s.kind == fm.DESPERATE:
                    continue
                if not (any(s.damage) or s.time_damage[0] or s.add_damage[0]):
                    continue
                w = weapon_for(s, job, side, level)
                if w is None:
                    rows.append({"skill": s, "weapon": "none fits"})
                    continue
                c = build(job, side, level, None if w[0] in (None, "shield") else w[0], w[0] == "shield")
                c = replace(c, attack_skills=[s])
                acts = model.actions_vs_mob(c, d)
                pvp = model.actions_vs_player(c, target)
                mine = next(a for a in acts if a.skill is not None)
                normal = next((a for a in acts if a.skill is None), None)
                pmine = next(a for a in pvp if a.skill is not None)
                pnormal = next((a for a in pvp if a.skill is None), None)
                weapon = next((fm.real_type(it.item_type) for it in c.build.items if it.slot == 5), 0)
                rows.append({"skill": s, "weapon": "shield" if w[0] == "shield" else REAL_NAMES.get(weapon, ""),
                             "pve": mine.damage + mine.tick * mine.ticks, "pve_ms": mine.time_ms,
                             "normal_pve_s": normal.damage / normal.time_ms * 1000 if normal else 0.0,
                             "pvp": pmine.damage + pmine.tick * pmine.ticks,
                             "normal_pvp_s": pnormal.damage / pnormal.time_ms * 1000 if pnormal else 0.0,
                             "target_hp": target.max_hp})
            for r in rows:
                s = r["skill"]
                row = {"class": (model.LIGHT if side == "light" else model.FURY)[job], "side": side, "level": level,
                       "id": s.id, "skill_level": s.level, "name": s.name, "weapon": r["weapon"], "sp": s.need_sp, "mp": s.need_mp,
                       "cast_ms": s.ready_time * 250, "cooldown_s": s.reset_time, "damage_type": DAMAGE_TYPES.get(s.damage_type, "")}
                if "pve" in r:
                    def worse(o):
                        q = o["skill"]
                        return (o is not r and "pve" in o and o["weapon"] == r["weapon"] and o["pve"] >= r["pve"] and o["pvp"] >= r["pvp"]
                                and o["pve_ms"] <= r["pve_ms"] and q.reset_time <= s.reset_time and q.need_sp <= s.need_sp
                                and q.need_mp <= s.need_mp and (o["pve"], o["pvp"]) != (r["pve"], r["pvp"]))
                    better = [o["skill"] for o in rows if worse(o)]
                    row.update({"mob": d.id, "pve_damage": round(r["pve"]), "pve_per_s": round(r["pve"] / r["pve_ms"] * 1000),
                                "normal_pve_per_s": round(r["normal_pve_s"]), "pvp_damage": round(r["pvp"]),
                                "pvp_per_s": round(r["pvp"] / r["pve_ms"] * 1000), "normal_pvp_per_s": round(r["normal_pvp_s"]),
                                "pvp_share_of_target_hp": round(r["pvp"] / r["target_hp"], 3),
                                "below_normal_pve": r["pve"] / r["pve_ms"] * 1000 < r["normal_pve_s"],
                                "below_normal_pvp": r["pvp"] / r["pve_ms"] * 1000 < r["normal_pvp_s"],
                                "dominated_by": " ".join(f"{q.id}/{q.level} {q.name}" for q in better)})
                out.append(row)
    return out


def skill_rows(with_model: bool) -> list:
    out = []
    names = data.skill_names()
    for (sid, level), s in sorted(data.skills().items()):
        if level == fm.MOB_LEVEL or s.type_show == 0:
            continue
        who = classes_of(s.jobs, s.country)
        cname = names.get((sid, level), "").strip()
        row = {
            "id": sid, "level": level, "name": cname or s.name, "name_from": "client" if cname else "defs",
            "jobs": jobs_letters(s.jobs), "country": s.country, "side": data.side_of_country(s.country), "classes": " ".join(who),
            "learnable": bool(who), "required_level": s.required_level, "lowest_mode": MODES[min(s.grow, 3)], "points": s.points,
            "effect": EFFECTS.get(s.type_effect, s.type_effect), "kind": s.kind, "what": KINDS.get((s.type_effect, s.kind), f"wType {s.kind}"),
            "built": not not_built(s), "attack_kind": ATTACK_KINDS[s.attack_kind] if s.type_effect in (1, 3) else "",
            "sp": "all" if s.need_sp == 1 else s.need_sp, "mp": "all" if s.need_mp == 1 else s.need_mp,
            "cast_ms": s.ready_time * 250, "cooldown_s": s.reset_time, "range": s.attack_range, "area": s.apply_range,
            "damage_type": DAMAGE_TYPES.get(s.damage_type, s.damage_type) if s.type_effect in (1, 3) and any(s.damage) else "",
            "damage_hp": s.damage[0], "damage_sp": s.damage[1], "damage_mp": s.damage[2], "add_damage": s.add_damage[0],
            "hits": s.multi_attack if s.kind == fm.MULTI_HIT else "", "dot_per_3s": s.time_damage[0] or "",
            "heal": s.heal[0] or "", "heal_over_time": s.time_heal[0] or "", "keep_s": s.keep_time, "abilities": ability_text(s),
            "success": f"type {s.success_type} {s.success_value}" if s.success_type else "", "state": s.state_type or "",
            "element": s.element or "", "weapons": " ".join(str(w + 1) for w, ok in enumerate(s.need_weapons) if ok) + (" shield" if s.need_shield else ""),
            "prev_skill": s.prev_skill or "",
        }
        if with_model and who and s.type_effect in (fm.ATTACK, fm.DEBUFF) and s.target_type in (3, 6, 7) and s.kind != fm.DESPERATE \
                and (any(s.damage) or s.time_damage[0] or s.add_damage[0]):
            job = next(j for j in range(6) if s.jobs[j])
            row.update(skill_numbers(s, job, max(min(s.required_level, 60), 1)))
        out.append(row)
    return out


# --- monsters ------------------------------------------------------------------------------------------------------------------------


def mob_rows(with_model: bool) -> list:
    out = []
    where = data.placed()
    raw = {_i(r["mobid"]): r for r in data.rows("mobs")}
    area_hp: dict = {}
    for m, places in where.items():
        d = data.mobs().get(m)
        if d and any(h == "area" for _, _, h in places):
            area_hp.setdefault(d.level, []).append(d.hp)
    for mid, d in sorted(data.mobs().items()):
        places = where.get(mid, [])
        e = d.attacks[0]
        aps = (e.damage + max(e.damage_add - 1, 0) / 2) * 1000 / e.time_ms if e.time_ms else 0
        hows = sorted({h for _, _, h in places})
        med = statistics.median(area_hp[d.level]) if area_hp.get(d.level) else 0
        row = {
            "id": mid, "name": d.name, "level": d.level, "hp": d.hp, "sp": d.sp, "mp": d.mp, "exp": d.exp,
            "exp_per_hp": round(d.exp / d.hp, 3) if d.hp else "", "defence": d.defense, "magic_defence": d.magic, "dex": d.dex,
            "luc": d.luc, "wis": d.wis, "element": d.attrib, "size": d.size, "ai": d.ai, "money": _i(raw[mid]["money1"]),
            "attack": e.damage, "attack_add": e.damage_add, "attack_ms": e.time_ms, "attack_per_s": round(aps, 1),
            "skills": " ".join(str(a.skill) for a in d.attacks if a.skill), "placed": bool(places),
            "count": sum(c for _, c, _ in places), "maps": " ".join(str(m) for m in sorted({m for m, _, _ in places})),
            "how": " ".join(hows), "hp_over_level_median": round(d.hp / med, 1) if med else "",
            "content": ("boss" if set(hows) & {"boss", "obelisk", "summoned"} else
                        "party (inferred)" if med and d.hp >= 5 * med else "solo" if places else ""),  # replaced below with the model
        }
        if with_model and places and d.hp > 0 and 1 <= d.level:
            lv = min(max(d.level, 1), 60)
            for job, label in ((0, "fighter"), (4, "mage")):
                c = build(job, "light", lv)
                if c is None:
                    continue
                r = model.vs_mob(c, d)
                row[f"{label}_kill_s"] = r["time_to_kill_unlimited_s"]
                row[f"{label}_dies_s"] = r["time_to_die_s"]
                row[f"{label}_hit"] = r["hit_rate_normal"]
            # With the model: solo content when the default fighter or mage of its level kills it (potions allowed) before dying.
            solo = any(row.get(f"{x}_kill_s", float("inf")) < row.get(f"{x}_dies_s", 0) for x in ("fighter", "mage"))
            if row["content"] != "boss":
                row["content"] = "solo" if solo else "party (modelled)"
        out.append(row)
    return out


def origin_rows() -> list:
    out = []
    for (t, tid), found in sorted(data.origins().items()):
        name, _ = item_name((t, tid))
        for o in found:
            out.append({"type": t, "typeid": tid, "name": name, "kind": o["kind"], "detail": o["detail"], "side": o["side"],
                        "level": o["level"] if o["level"] is not None else "", "chance": f"{o['chance']:.3g}",
                        "one_in": one_in(o["chance"]).removeprefix("1 in ") if o["chance"] else ""})
    return out


# --- the client's copy ---------------------------------------------------------------------------------------------------------------

CLIENT_ITEM_SQL = """SELECT d.*, c.reqlevel AS c_reqlevel, c.country AS c_country, c.attackfighter AS c_attackfighter,
    c.defensefighter AS c_defensefighter, c.patrolrogue AS c_patrolrogue, c.shootrogue AS c_shootrogue, c.attackmage AS c_attackmage,
    c.defensemage AS c_defensemage, c.grow AS c_grow, c.reqstr AS c_reqstr, c.reqdex AS c_reqdex, c.reqrec AS c_reqrec,
    c.reqint AS c_reqint, c.reqwis AS c_reqwis, c.reqluc AS c_reqluc, c.reqvg AS c_reqvg, c.reqog AS c_reqog, c.reqig AS c_reqig,
    c.range AS c_range, c.attacktime AS c_attacktime, c.attrib AS c_attrib, c.special AS c_special, c.slot AS c_slot,
    c.quality AS c_quality, c.effect1 AS c_effect1, c.effect2 AS c_effect2, c.effect3 AS c_effect3, c.effect4 AS c_effect4,
    c.consthp AS c_consthp, c.constsp AS c_constsp, c.constmp AS c_constmp, c.conststr AS c_conststr, c.constdex AS c_constdex,
    c.constrec AS c_constrec, c.constint AS c_constint, c.constwis AS c_constwis, c.constluc AS c_constluc, c.speed AS c_speed,
    c.exp AS c_exp, c.buy AS c_buy, c.sell AS c_sell, c.server AS c_server, c.count AS c_count
    FROM defs.items d JOIN client.items c ON c.type = d.type AND c.typeid = d.typeid"""
# Fields of a different width in the client (docs/client/item-sdata-vs-defs.md section 2): Range is one byte there; grade and drop are
# the server's own and not compared.
NARROW = {"range": 0xFF}
WIDE = {"buy", "sell"}


def client_item_rows(items: list) -> list:
    by_key = {(r["type"], r["typeid"]): r for r in items}
    lapis = {(30, r["typeid"]) for r in lapis_rows_cache()}
    rows = data._query_sql("client_items_full", CLIENT_ITEM_SQL)
    out = []
    for r in rows:
        key = (_i(r["type"]), _i(r["typeid"]))
        if key not in by_key and key not in lapis:
            continue
        fields = [k[2:] for k in r if k.startswith("c_")]
        cvals = [_i(r["c_" + f]) for f in fields]
        if all(v == 0 for v in cvals):
            continue  # an empty client record (docs/client/item-sdata-vs-defs.md section 1)
        diffs = []
        for f, cv in zip(fields, cvals):
            sv = _i(r[f])
            mask = NARROW.get(f, None if f in WIDE else 0xFFFF)
            if (sv & mask if mask else sv) != (cv & mask if mask else cv):
                diffs.append(f"{f} {sv & mask if mask else sv} / {cv & mask if mask else cv}")
        if not diffs:
            continue
        it = by_key.get(key)
        obt = it["obtainable"] if it else any(o["kind"] not in model.RARE for o in data.origins().get(key, []))
        out.append({"type": key[0], "typeid": key[1], "name": item_name(key)[0], "obtainable": obt,
                    "usable": it["usable"] if it else "", "differences (server / client)": "; ".join(diffs)})
    return out


@lru_cache(maxsize=None)
def lapis_rows_cache():
    return tuple(lapis_rows())


SKILL_RENAME = {"prev_skill": "prevskillid", "defense_type": "defencetype", "defense_value": "defencevalue", "attr_type": "attrtype",
                "req_level": "reqlevel", "skill_point": "skillpoint", "fix_range": "fixrange"}
SKILL_CLIENT_ONLY = {"skill_id", "slot", "name", "description", "level", "icon", "animation", "toggle_type", "sound"}


def client_skill_rows(skills: list) -> list:
    sql = "SELECT * FROM client.skills"
    crows = data._query_sql("client_skills_full", sql)
    server = {(_i(r["skillid"]), _i(r["skilllevel"])): r for r in data.rows("skills")}
    learn = {(r["id"], r["level"]): r for r in skills}
    out = []
    for c in crows:
        key = (_i(c["skill_id"]) + 0, _i(c["level"]))
        # client.skills keeps the skill id itself (the loader adds 1 to the group); compare by id and level.
        s = server.get(key)
        if s is None or key not in learn:
            continue
        diffs = []
        for f in c:
            if f in SKILL_CLIENT_ONLY:
                continue
            df = SKILL_RENAME.get(f, f.replace("_", ""))
            if df not in s:
                continue
            mask = 0xFFFFFFFF if f == "keep_time" else 0xFFFF
            if _i(s[df]) & mask != _i(c[f]) & mask:
                diffs.append(f"{f} {_i(s[df]) & mask} / {_i(c[f]) & mask}")
        if diffs:
            hours = {d.split()[0] for d in diffs} <= {"keep_time", "fix_range"} and _i(s["fixrange"]) == 2
            out.append({"id": key[0], "level": key[1], "name": learn[key]["name"], "classes": learn[key]["classes"],
                        "only_hours_vs_seconds": hours, "differences (server / client)": "; ".join(diffs)})
    return out


# --- main ----------------------------------------------------------------------------------------------------------------------------


def _query_sql(name: str, sql: str) -> list:
    """A cached ad-hoc query (as data.rows)."""
    data.QUERIES.setdefault(name, sql)
    return data.rows(name)


data._query_sql = _query_sql

PARTS = ("items", "lapis", "tools", "enchant", "skills", "mobs", "origins", "client")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--refresh", action="store_true")
    ap.add_argument("--only", default=",".join(PARTS))
    ap.add_argument("--quick", action="store_true", help="leave out the model's numbers")
    args = ap.parse_args(argv)
    if args.refresh:
        data.refresh()
        for name in ("client_items_full", "client_skills_full"):
            path = data.CACHE / f"{name}.csv"
            if path.exists():
                path.unlink()
    only = set(args.only.split(","))
    items = item_rows()
    if "items" in only:
        write("items.csv", items)
        write("item_brackets.csv", bracket_rows(items))
        write("item_pairs.csv", pair_rows(items))
        dups = duplicate_rows(items)
        write("item_duplicates.csv", [{"group": i, "type": r["type"], "typeid": r["typeid"], "name": r["name"], "level": r["level"],
                                       "kind": r["kind"], "obtainable": r["obtainable"]} for i, g in enumerate(dups) for r in g])
    if "items" in only:
        write("item_skills.csv", item_skill_rows())
    if "lapis" in only:
        write("lapis.csv", list(lapis_rows_cache()))
    tools = tool_rows(items)
    if "tools" in only:
        write("tools.csv", tools)
    if "enchant" in only:
        write("enchant.csv", enchant_rows())
    skills = None
    if "skills" in only or "client" in only:
        skills = skill_rows(not args.quick and "skills" in only)
    if "skills" in only:
        fields = list(dict.fromkeys(k for r in skills for k in r))
        write("skills.csv", skills, fields)
        if not args.quick:
            for level in (15, 30, 60):
                rows = class_skill_rows(level)
                write(f"class_skills_{level}.csv", rows, list(dict.fromkeys(k for r in rows for k in r)))
    if "mobs" in only:
        mobs = mob_rows(not args.quick)
        write("mobs.csv", mobs, list(dict.fromkeys(k for r in mobs for k in r)))
    if "origins" in only:
        write("origins.csv", origin_rows())
    if "client" in only:
        write("client_items.csv", client_item_rows(items))
        write("client_skills.csv", client_skill_rows(skills))


if __name__ == "__main__":
    main()
