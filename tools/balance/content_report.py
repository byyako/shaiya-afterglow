#!/usr/bin/env python3
"""The tables of docs/plans/balance/2-content.md, printed as Markdown from the CSVs content.py builds in data/balance/ (run
content.py first). Each section of the output names the document section it feeds.

    python3 content_report.py [--section items,weapons,...]
"""
from __future__ import annotations

import argparse
import collections
import csv
import statistics

import data
import model
from content import OUT, REAL_NAMES, classes_of, named, reference_mob

LIGHT, FURY = model.LIGHT, model.FURY
WEAPON_KINDS = [REAL_NAMES[r] for r in range(1, 16)]
ARMOUR_KINDS = ["helmet", "upper armour", "lower armour", "gloves", "boots", "shield"]
JEWEL_KINDS = ["ring", "amulet", "bracelet", "cloak"]


def load(name: str) -> list:
    with (OUT / name).open(newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def t(v) -> bool:
    return v == "True"


def num(v, default=0.0) -> float:
    try:
        return float(v)
    except (TypeError, ValueError):
        return default


def table(head: list, rows: list) -> None:
    print("| " + " | ".join(head) + " |")
    print("|" + "---|" * len(head))
    for r in rows:
        print("| " + " | ".join(str(x) for x in r) + " |")
    print()


def kinds(r) -> set:
    """The origin kinds of an `items.csv` row (its `origins` joins them with spaces; two kinds have two words)."""
    return set(r["origins"].replace("mall box", "mall_box").replace("sequence box", "sequence_box").split())


def items() -> list:
    return [r for r in load("items.csv") if named(r["name"])]


# --- sections ------------------------------------------------------------------------------------------------------------------------


def s_counts():
    print("### items by kind (section 3.1)\n")
    rows = []
    for kind in WEAPON_KINDS + ARMOUR_KINDS + JEWEL_KINDS:
        rs = [r for r in items() if r["kind"] == kind]
        rows.append([kind, len(rs), sum(t(r["usable"]) for r in rs), sum(t(r["obtainable"]) for r in rs),
                     sum(1 for r in rs if not t(r["obtainable"]) and r["origins"] and kinds(r) <= {"mall", "mall_box"}),
                     sum(1 for r in rs if not t(r["obtainable"]) and r["origins"] and not kinds(r) <= {"mall", "mall_box"}),
                     sum(1 for r in rs if not r["origins"])])
    rows.append(["all"] + [sum(r[i] for r in rows) for i in range(1, 7)])
    table(["kind", "named rows", "usable", "obtainable", "mall or mall box only", "sequence (and mall) only", "no origin"], rows)
    every = load("items.csv")
    print(f"All equipment rows: {len(every)}; placeholders (no name, `???`, `...`, `??`, `…`): {len(every) - len(items())}.\n")
    by_mode = collections.Counter((r["lowest_mode"], t(r["obtainable"])) for r in items() if t(r["usable"]))
    table(["lowest mode", "usable rows", "obtainable"], [[m, by_mode[(m, True)] + by_mode[(m, False)], by_mode[(m, True)]]
                                                         for m in model.MODES])


def s_origins():
    print("### origin kinds (section 2)\n")
    o = load("origins.csv")
    c = collections.Counter(r["kind"] for r in o)
    per_item = collections.Counter(k for r in {(x["type"], x["typeid"], x["kind"]) for x in o} for k in [r[2]])
    table(["kind", "origins (rows)", "items"], [[k, c[k], per_item[k]] for k in sorted(c, key=lambda k: -c[k])])


def best_by_class(rs, level, kinds, field="value"):
    out = {}
    for r in rs:
        if r["kind"] in kinds and int(r["level"]) <= level:
            key = r["kind"]
            if key not in out or num(r[field]) > num(out[key][field]):
                out[key] = r
    return out


def class_items(cls: str, obtainable=True):
    return [r for r in items() if cls in r["classes"].split() and t(r["usable"]) and (t(r["obtainable"]) or not obtainable)]


def s_weapons():
    print("### best obtainable weapon per class (section 3.3)\n")
    for level in (15, 30, 60):
        rows = []
        for cls in LIGHT + FURY:
            best = best_by_class(class_items(cls), level, WEAPON_KINDS)
            top = sorted(best.values(), key=lambda r: -num(r["value"]))[:3]
            allbest = best_by_class(class_items(cls, False), level, WEAPON_KINDS)
            topall = max(allbest.values(), key=lambda r: num(r["value"])) if allbest else None
            rows.append([cls] + [f"{r['name'].strip()} ({r['kind']}, L{r['level']}, {r['lowest_mode']}): {num(r['mean_attack']):.0f}, "
                                 f"{r['attack_per_s']}/s{', ' + stats(r) if stats(r) else ''}" for r in top[:2]]
                        + [f"{num(topall['mean_attack']):.0f} ({topall['origins'] or 'none'})" if topall else "-"])
        print(f"Level {level} or below, ultimate (any mode):\n")
        table(["class", "best", "second (another kind)", "best of any origin: mean attack (its origins)"], rows)


def stats(r) -> str:
    return " ".join(f"{s.upper()} {r[s]}" for s in ("str", "dex", "rec", "int", "wis", "luc") if num(r[s]))


def s_armour():
    print("### the best obtainable armour set per class (section 3.4)\n")
    rows = []
    for level in (15, 30, 60):
        for cls in LIGHT + FURY:
            best = best_by_class(class_items(cls), level, ARMOUR_KINDS[:5])
            pieces = list(best.values())
            rows.append([level, cls, len(pieces), sum(int(r["defence"]) for r in pieces), sum(int(r["magic_defence"]) for r in pieces),
                         sum(int(r["hp"]) for r in pieces), sum(int(r["mp"]) for r in pieces), sum(int(r["sp"]) for r in pieces),
                         round(sum(num(r["stat_budget"]) for r in pieces)), round(sum(num(r["craft_expected"]) for r in pieces)),
                         round(sum(num(r["craft_highest"]) for r in pieces))])
    table(["level", "class", "pieces", "defence", "magic defence", "HP", "MP", "SP", "stat points", "crafted (expected)",
           "crafted (highest)"], rows)


def s_brackets():
    print("### the main number by kind and level (section 3.2)\n")
    marks = [5, 15, 25, 30, 40, 50, 55, 60]
    pool = [r for r in items() if t(r["usable"]) and t(r["obtainable"])]
    rows = []
    for kind in WEAPON_KINDS + ARMOUR_KINDS + JEWEL_KINDS:
        cells = []
        for m in marks:
            v = [num(r["value"]) for r in pool if r["kind"] == kind and m - 5 < int(r["level"]) <= m]
            cells.append(f"{statistics.median(v):.0f} ({len(v)})" if v else "-")
        rows.append([kind] + cells)
    print("Median of the obtainable, usable items of levels m − 4 to m (how many): weapons mean attack; armour and shields defence +"
          " magic defence + stat points; accessories and cloaks stat points + effects.\n")
    table(["kind"] + [f"{m - 4}-{m}" for m in marks], rows)


def s_outliers():
    print("### outliers among obtainable items (section 3.6)\n")
    rs = [r for r in items() if r["outlier"] and t(r["obtainable"])]
    c = collections.Counter((r["kind"], r["outlier"]) for r in rs)
    table(["kind", "high", "low"], [[k, c[(k, "high")], c[(k, "low")]] for k in WEAPON_KINDS + ARMOUR_KINDS + JEWEL_KINDS
                                    if c[(k, "high")] or c[(k, "low")]])
    for side in ("high", "low"):
        top = sorted((r for r in rs if r["outlier"] == side), key=lambda r: -num(r["ratio"]) if side == "high" else num(r["ratio"]))
        seen, rows = set(), []
        for r in top:
            key = (r["name"].strip(), r["level"])
            if key in seen:
                continue
            seen.add(key)
            rows.append([f"{r['type']}/{r['typeid']} {r['name'].strip()}", r["kind"], r["level"], r["lowest_mode"], r["value"],
                         r["peer_median"], r["ratio"], r["origins"]])
            if len(rows) == 15:
                break
        print(f"The {side}est (one row per name and level):\n")
        table(["item", "kind", "level", "mode", "value", "peers' median", "ratio", "origins"], rows)


def s_pairs():
    print("### Light and Fury items (section 3.7)\n")
    p = load("item_pairs.csv")
    c = collections.Counter(r["status"] for r in p)
    both = [r for r in p if t(r["light_obtainable"]) and t(r["fury_obtainable"])]
    cb = collections.Counter(r["status"] for r in both)
    table(["status", "pairs", "both obtainable"], [[s, c[s], cb[s]] for s in ("mirrored", "differs", "no Fury counterpart",
                                                                               "no Light counterpart")])
    agg = collections.defaultdict(lambda: [0, 0, 0.0, 0.0])
    for r in p:
        if r["status"] != "differs":
            continue
        for x in r["differences"].split("; "):
            f, v = x.split(" ", 1)
            a, b = v.split("/")
            if f == "jobs":
                agg[(r["kind"], "jobs " + v)][0] += 1
                continue
            try:
                a, b = float(a), float(b)
            except ValueError:
                continue
            k = agg[(r["kind"], f)]
            k[0 if a > b else 1] += 1
            k[2] += a
            k[3] += b
    rows = [[k[0], k[1], v[0], v[1], f"{v[2]:.0f} / {v[3]:.0f}" if not k[1].startswith("jobs") else ""]
            for k, v in sorted(agg.items(), key=lambda kv: -(kv[1][0] + kv[1][1])) if v[0] + v[1] >= 3]
    table(["kind", "number", "Light higher (or pairs)", "Fury higher", "sum Light / Fury"], rows)


def s_duplicates():
    print("### duplicates (section 3.8)\n")
    d = load("item_duplicates.csv")
    groups = collections.defaultdict(list)
    for r in d:
        groups[r["group"]].append(r)
    c = collections.Counter(g[0]["kind"] for g in groups.values())
    print(f"{len(groups)} groups, {len(d)} rows; with an obtainable member: "
          f"{sum(1 for g in groups.values() if any(t(r['obtainable']) for r in g))}.\n")
    table(["kind", "groups"], c.most_common())


def s_client():
    print("### the client's copy (section 3.9)\n")
    ci = load("client_items.csv")
    ob = [r for r in ci if t(r["obtainable"])]
    fields = collections.Counter(x.split()[0] for r in ob for x in r["differences (server / client)"].split("; "))
    print(f"Equipment and lapis rows that differ: {len(ci)}; obtainable: {len(ob)}. Fields among the obtainable: "
          + ", ".join(f"{f} {n}" for f, n in fields.most_common()) + ".\n")
    cs = load("client_skills.csv")
    real = [r for r in cs if not t(r["only_hours_vs_seconds"])]
    table(["skill", "level", "name", "classes", "server / client"],
          [[r["id"], r["level"], r["name"], r["classes"], r["differences (server / client)"]] for r in real])


def s_lapis():
    print("### lapis (section 4.1)\n")
    rows = []
    for r in load("lapis.csv"):
        if not named(r["name"]):
            continue
        eff = " ".join(f"{k} {r[k]}" for k in ("attack", "spread", "defence", "magic_defence", "str", "dex", "rec", "int", "wis", "luc",
                                               "hp", "mp", "sp", "attack_speed", "move_speed", "range", "absorption", "element")
                       if r[k] not in ("0", ""))
        rows.append([r["typeid"], r["name"].strip(), r["grade"], r["fits"], "yes" if t(r["dangerous"]) else "", eff,
                     r["option_skill"] + (f" ({r['option_what']})" if r["option_what"] else ""), r["price"], r["link_60"],
                     r["origins"] or "none", (r["best_drop"].split(" (")[0] + " " + r["best_drop"].rsplit(") ", 1)[-1]) if r["best_drop"] else ""])
    table(["id", "name", "grade", "fits", "dangerous", "adds", "option skill", "price", "link % at 60", "origins", "best drop"], rows)


def s_tools():
    print("### Lapisia, hammers, charms, runes (section 4.2)\n")
    rows = []
    for r in load("tools.csv"):
        if not named(r["name"]) or r["what"] == "box" or r["what"].startswith("enchant material"):
            continue
        rows.append([f"{r['type']}/{r['typeid']} {r['name'].strip()}", r["what"], r["grade"] or "", r["reqvg"], r["fits"],
                     r["origins"] or "none", r["best_drop"]])
    table(["item", "what", "grade", "reqvg", "fits", "origins", "best drop"], rows)
    mats = [r for r in load("tools.csv") if "enchant material" in r["what"]]
    print(f"Enchant materials: {len(mats)} items; sold by a merchant: {sum('shop' in r['origins'] for r in mats)}; with no origin: "
          f"{sum(not r['origins'] for r in mats)}.\n")
    table(["material", "used by", "origins", "best drop", "price"],
          [[f"{r['type']}/{r['typeid']} {r['name'].strip()}", r["what"].replace("enchant material of ", ""), r["origins"] or "none",
            r["best_drop"], r["buy"]] for r in sorted(mats, key=lambda r: -int(r["what"].split()[3]))[:20]])


def s_enchant():
    print("### enchanting (section 4.3)\n")
    rows = []
    for r in load("enchant.csv")[:14]:
        rows.append([r["to_step"], r["weapon_bonus"], r["weapon_chance"], r["weapon_safe_tries"], r["weapon_dangerous_tries"],
                     r["weapon_dangerous_items"], r["armour_bonus"], r["armour_chance"], r["armour_safe_tries"],
                     r["armour_dangerous_tries"], r["armour_dangerous_items"]])
    table(["to step", "weapon attack", "weapon chance", "safe tries from 0", "dangerous Lapisia from 0", "dangerous weapons used",
           "armour absorption", "armour chance", "safe tries from 0", "dangerous Lapisia from 0", "dangerous pieces used"], rows)


def s_skills():
    print("### skills (section 5)\n")
    s = load("skills.csv")
    c = collections.Counter()
    for r in s:
        if not t(r["learnable"]):
            continue
        for cls in r["classes"].split():
            c[(cls, r["effect"])] += 1
    effects = ["attack", "debuff", "buff", "heal", "passive", "field", "one-off"]
    table(["class"] + effects + ["rows"], [[cls] + [c[(cls, e)] for e in effects] + [sum(c[(cls, e)] for e in effects)]
                                           for cls in LIGHT + FURY])
    table(["id", "levels", "name", "why"], unusable_skills(s))


def unusable_skills(s):
    rows = []
    by_id = collections.defaultdict(list)
    for r in s:
        by_id[r["id"]].append(r)
    for sid, rs in sorted(by_id.items(), key=lambda kv: int(kv[0])):
        why = []
        if any(not t(r["learnable"]) for r in rs):
            levels = ",".join(r["level"] for r in rs if not t(r["learnable"]))
            why.append(f"levels {levels}: no class may learn them (no job flag)")
        if any(not t(r["built"]) for r in rs):
            why.append("refused as not built (`SkillDef::not_built`)")
        if any(r["what"].startswith("language") for r in rs):
            why.append("passive kind 5, language: nothing in the server reads it")
        if why:
            rows.append([sid, ",".join(r["level"] for r in rs), rs[0]["name"].replace(" Lv1", "").strip(), "; ".join(why)])
    return rows


def s_class_skills():
    for level in (15, 30, 60):
        rows = load(f"class_skills_{level}.csv")
        d = reference_mob(level)
        print(f"### attack skills at level {level} (section 5.2): monster {d.id} (level {d.level}, HP {d.hp}), the other side's mirror\n")
        out = []
        for cls in LIGHT + FURY:
            rs = [r for r in rows if r["class"] == cls and r.get("pve_damage")]
            if not rs:
                continue
            best_pve = max(rs, key=lambda r: num(r["pve_per_s"]))
            best_pvp = max(rs, key=lambda r: num(r["pvp_per_s"]))
            below = [r for r in rs if t(r["below_normal_pve"]) and t(r["below_normal_pvp"])]
            dom = [r for r in rs if r["dominated_by"]]
            out.append([cls, len(rs), rs[0]["normal_pve_per_s"], f"{best_pve['name'].strip()} {best_pve['pve_per_s']}",
                        rs[0]["normal_pvp_per_s"], f"{best_pvp['name'].strip()} {best_pvp['pvp_per_s']} ({num(best_pvp['pvp_share_of_target_hp']):.0%})",
                        len(below), len(dom)])
        table(["class", "attack skills", "normal on monster /s", "best on monster /s", "normal on player /s",
               "best on player /s (share of HP a use)", "below the normal blow on both", "another skill as good in every way"], out)


def s_never():
    rows = load("class_skills_60.csv")
    print("### attack skills at 60 that are never better (section 5.3)\n")
    out = []
    for r in rows:
        if r["side"] != "light" or not r.get("pve_damage"):
            continue
        if (t(r["below_normal_pve"]) and t(r["below_normal_pvp"])) or r["dominated_by"]:
            out.append([r["class"], f"{r['id']}/{r['skill_level']} {r['name'].strip()}", r["pve_per_s"], r["normal_pve_per_s"],
                        r["pvp_per_s"], r["normal_pvp_per_s"], r["dominated_by"] or "-"])
    table(["class", "skill", "on monster /s", "normal", "on player /s", "normal", "as good in every way"], out)


def s_percent():
    print("### percentage skills and monster resistances (section 5.4)\n")
    sk = data.skills()
    mobs, placed = data.mobs(), data.placed()
    rows = []
    for (sid, lv), s in sorted(sk.items()):
        if lv not in (1, 2, 3) or not s.type_show or s.damage_type not in (3, 4, 5) or not any(s.damage):
            continue
        cls = " ".join(classes_of(s.jobs, s.country))
        if not cls:
            continue
        resist = [m for m in placed if m in mobs and ((s.state_type and mobs[m].resist_states[s.state_type - 1]) or sid in mobs[m].resist_skills)]
        cap = [m for m in placed if m in mobs and mobs[m].level >= 51]
        cap_resist = [m for m in resist if mobs[m].level >= 51]
        rows.append([f"{sid}/{lv} {s.name.strip()}", cls, s.required_level, model.MODES[min(s.grow, 3)],
                     {3: "× caster's current", 4: "% of max", 5: "% of current"}[s.damage_type],
                     "/".join(str(x) for x in s.damage), f"type {s.success_type} {s.success_value}" if s.success_type else "roll",
                     s.reset_time, s.state_type or "", f"{len(resist)} / {len(placed)}", f"{len(cap_resist)} / {len(cap)}"])
    table(["skill", "classes", "level", "mode", "damage", "HP/SP/MP values", "success", "cooldown s", "state",
           "placed monsters resisting", "of them level 51+"], rows)


def s_mobs():
    print("### monsters by level (section 6)\n")
    m = [r for r in load("mobs.csv") if t(r["placed"])]
    bands = [(1, 5), (6, 10), (11, 15), (16, 20), (21, 25), (26, 30), (31, 35), (36, 40), (41, 45), (46, 50), (51, 55), (56, 58),
             (59, 60), (61, 64), (65, 99)]
    rows = []
    for lo, hi in bands:
        rs = [r for r in m if lo <= int(r["level"]) <= hi and int(r["exp"]) > 0]
        solo = [r for r in rs if r["content"] != "boss"]
        if not rs:
            continue

        def med(k, pool):
            v = [num(r[k], None) for r in pool if num(r.get(k), None) is not None]
            v = [x for x in v if x is not None]
            return statistics.median(v) if v else None

        def f(x, d=0):
            return "-" if x is None else "never" if x == float("inf") else f"{x:.{d}f}"

        rows.append([f"{lo}-{hi}", len(rs), len(solo), len([r for r in rs if r["content"] == "party (modelled)"]),
                     len([r for r in rs if r["content"] == "boss"]), f(med("hp", solo)), f(med("defence", solo)),
                     f(med("magic_defence", solo)), f(med("dex", solo)), f(med("attack_per_s", solo)), f(med("exp_per_hp", solo), 2),
                     f(med("fighter_kill_s", solo), 1), f(med("fighter_dies_s", solo), 1), f(med("mage_kill_s", solo), 1),
                     f(med("mage_dies_s", solo), 1)])
    print("Placed monsters with experience (kinds, not counts placed); medians over the kinds that are not bosses. Solo: the default"
          " fighter or mage of the level (part 1's builds, capped at 60) kills it before dying, potions allowed; party: neither does.\n")
    table(["levels", "kinds", "not boss", "of them party", "boss", "HP", "defence", "magic defence", "DEX", "attack/s", "exp per HP",
           "fighter kills (s)", "fighter dies (s)", "mage kills (s)", "mage dies (s)"], rows)


SECTIONS = {"counts": s_counts, "origins": s_origins, "brackets": s_brackets, "weapons": s_weapons, "armour": s_armour,
            "outliers": s_outliers, "pairs": s_pairs, "duplicates": s_duplicates, "client": s_client, "lapis": s_lapis,
            "tools": s_tools, "enchant": s_enchant, "skills": s_skills, "class_skills": s_class_skills, "never": s_never,
            "percent": s_percent, "mobs": s_mobs}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--section", default=",".join(SECTIONS))
    args = ap.parse_args()
    for name in args.section.split(","):
        SECTIONS[name]()


if __name__ == "__main__":
    main()
