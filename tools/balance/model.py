#!/usr/bin/env python3
"""The balance model: a character's numbers and how it fights, from the game's own formulas (formulas.py, a port of the server's code)
and data (data.py, read-only from PostgreSQL).

    python3 model.py char  "fighter human ultimate 60 alloc=str:70,dex:30"
    python3 model.py mob   "fighter human ultimate 60" --mob 2179
    python3 model.py pvp   "fighter human ultimate 60" "mage elf ultimate 60 alloc=int:80,wis:20"
    python3 model.py curve [--mode ultimate]

A build is a string of words: the class (Light or Fury name, or job=0..5), the race, the mode, the level, and optional key=value parts:
  alloc=str:70,dex:30     where the free stat points go (shares; default: the class's damage stat, see DEFAULT_ALLOC)
  stats=a,b,c,d,e,f       base STR,DEX,REC,INT,WIS,LUC instead of alloc
  gear=auto|none          auto: the rough picker below (default); none: naked
  weapon=6                the weapon real type the picker should take (default: the one giving the most attack)
  enchant=20              the weapon's enchant step (armour steps: armour=10)
  item=slot:type/typeid   an item put in a slot (repeat); lapis with item=5:6/40+30/1+30/1 (gems after +), then @3 its enchant step
                          and #s,d,r,i,w,l,hp,mp,sp its crafted numbers (item=5:6/203+30/79@3#10,5,0,0,0,0,0,0,0)
  craft=exp|good|top      crafted numbers on every craftable item: the item's count of numbers, on craftorder= (default: the class's
                          wants, DEFAULT_WANTS), each the mean, the tenth-best or the highest of the smallest of three draws
  lapis=35[:str,hp]       every empty lapis room (not the cloak) filled with the best linkable lapis of grade 31 to 35 by the wants
  element=1[,1]           a basic element lapis (grade 34) in the weapon and the upper armour, before lapis= fills the other rooms
  skills=auto|none        auto: every skill the class can learn at this level and mode, at its highest level (default)
  without=180,174         skill ids to leave out
  buffs=id/level,...      applied skills (wType 82/91 abilities, element buffs)
  grade=N                 the faction's kill-count grade (0 to 30)
  mall=1                  let the gear picker take item-mall items too
Output is text, or JSON with --json. Expected values throughout; see README.md for what is modelled and what is not.
"""
from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass, field
from functools import lru_cache

import data
import expect as ex
import formulas as fm

LIGHT = ["fighter", "defender", "ranger", "archer", "mage", "priest"]
FURY = ["warrior", "guardian", "assassin", "hunter", "pagan", "oracle"]
RACES = ["human", "elf", "vail", "deatheater"]
MODES = ["easy", "normal", "hard", "ultimate"]
STAT_NAMES = ["str", "dex", "rec", "int", "wis", "luc"]
# The usual first race for each class on each side (`creation::RACE_CLASSES`): Light human 0 1 5, elf 2 3 4; Fury deatheater 0 1 3,
# vail 2 4 5.
DEFAULT_RACE = {("light", 0): 0, ("light", 1): 0, ("light", 5): 0, ("light", 2): 1, ("light", 3): 1, ("light", 4): 1,
                ("fury", 0): 3, ("fury", 1): 3, ("fury", 3): 3, ("fury", 2): 2, ("fury", 4): 2, ("fury", 5): 2}
# Where the free points go by default: the stat each class's damage formula reads (combat::player): melee STR, ranged STR (with
# LUC × 0.3), magic INT; the defender and priest take REC and WIS. A starting point only; part 3 compares builds.
DEFAULT_ALLOC = {0: {"str": 1}, 1: {"str": 1}, 2: {"str": 1}, 3: {"str": 1}, 4: {"int": 1}, 5: {"int": 1}}
SLOT_REAL_TYPES = {0: [16], 1: [17], 2: [18], 3: [20], 4: [21], 5: list(range(1, 16)), 6: [19], 7: [24], 8: [23], 9: [22], 10: [22],
                   11: [25], 12: [25]}
ONE_HANDED = (1, 3, 7, 9)


def faction_of(family: int) -> int:
    return 0 if family < 2 else 1


def class_name(job: int, family: int) -> str:
    return (LIGHT if faction_of(family) == 0 else FURY)[job]


# --- rules ---------------------------------------------------------------------------------------------------------------------------


def race_fits(country: int, family: int) -> bool:
    """`byCountry` against the race (items and skills): 0 human, 1 elf, 2 human or elf, 3 deatheater, 4 vail, 5 vail or deatheater."""
    return {0: family == 0, 1: family == 1, 2: family <= 1, 3: family == 3, 4: family == 2, 5: family >= 2}.get(country, True)


def grow_fits(item_grow: int, grow: int) -> bool:
    """`EnableEquipment` / `EnableLearnSkill`: easy only 0, normal 0 or 1, hard anything but 3, ultimate anything."""
    return {0: item_grow == 0, 1: item_grow <= 1, 2: item_grow != 3}.get(grow, True)


def may_equip(d: fm.ItemDef, job: int, family: int, grow: int, level: int) -> bool:
    return d.level <= level and race_fits(d.country, family) and d.jobs[job] and grow_fits(d.grow, grow)


def may_learn(s: fm.SkillDef, job: int, family: int, grow: int, level: int) -> bool:
    return (s.type_show != 0 and s.required_level <= level and race_fits(s.country, family) and s.jobs[job] and grow_fits(s.grow, grow)
            and s.level in (1, 2, 3))


# --- builds --------------------------------------------------------------------------------------------------------------------------


@dataclass
class Build:
    job: int
    family: int
    grow: int
    level: int
    stats: list
    items: list
    learnt: list  # (id, level)
    buffs: list = field(default_factory=list)  # (id, level)
    grade: int = 0
    label: str = ""


def alloc_stats(job: int, family: int, grow: int, level: int, shares: dict) -> list:
    """A fresh build at this level (`item_use::reset_stats`) with its free points split by `shares` (whole points, the rest to the
    largest share)."""
    points, stats = fm.reset_stats(job, family, grow, level)
    total = sum(shares.values()) or 1
    given = 0
    for name, share in shares.items():
        n = int(points * share / total)
        stats[STAT_NAMES.index(name)] += n
        given += n
    if shares:
        stats[STAT_NAMES.index(max(shares, key=shares.get))] += points - given
    return stats


def auto_skills(job: int, family: int, grow: int, level: int) -> list:
    """Every learnable skill at its highest level allowed here (skill points are reported, not enforced)."""
    best = {}
    for (sid, lvl), s in data.skills().items():
        if may_learn(s, job, family, grow, level) and lvl > best.get(sid, 0):
            best[sid] = lvl
    return sorted(best.items())


def skill_points_used(learnt) -> int:
    sk = data.skills()
    return sum(sk[(sid, lv)].points for sid, top in learnt for lv in range(1, top + 1) if (sid, lv) in sk)


def _item_score(d: fm.ItemDef, slot: int, weights: list) -> float:
    """The rough picker's value of an item: attack (spread counted half) for weapons, defence and magic defence for armour, every
    stat by the build's weights, HP / 5. Part 2 rates items properly; this only picks a plausible set."""
    stats = sum(w * v for w, v in zip(weights, d.const_stats))
    health = d.const_health[0] / 5.0 + (d.const_health[1] + d.const_health[2]) / 20.0
    e1, e2, e3, e4 = d.effects
    if slot == 5:
        return e1 + max(e2 - 1, 0) / 2.0 + stats + health
    return e1 + max(e2 - 1, 0) / 2.0 + e3 + e4 * 0.6 + stats + health


# Origins too rare for a build: sequence items come from one monster in 16,001 to 30,000,001 made (`data.origins`).
RARE = {"sequence", "sequence box"}


def obtainable(d: fm.ItemDef, mall: bool = False) -> bool:
    """Whether a player can get the item in the world (`data.sources`): a shop, a drop, a box, a quest or the start; the mall too with
    `mall`. Sequence items do not count (`RARE`)."""
    how = data.sources().get((d.item_type, d.type_id), set()) - RARE
    return bool(how - {"mall", "mall box"}) or (mall and bool(how))


def auto_gear(job: int, family: int, grow: int, level: int, shares: dict, weapon_type=None, enchant=0, armour=0, shield=None,
              mall=False, extra=None) -> list:
    """The best-scoring item per slot that this character may equip and can get (`obtainable`; no "???" placeholders). A shield only
    with a one-handed weapon (`EnableEquipment`), taken when `shield` is true or, by default, for the defender. `extra(d, slot,
    weights)` adds to an item's score (the lapis its rooms will hold and its crafted numbers, `parse_build`)."""
    defs = data.items()
    weights = [0.6] * 6
    for name, share in shares.items():
        weights[STAT_NAMES.index(name)] += 1.3 * share / (sum(shares.values()) or 1)
    best = {}
    for d in defs.by_key.values():
        real = fm.real_type(d.item_type)
        if real == 0 or not may_equip(d, job, family, grow, level) or d.name.startswith(("???", "...")) or not obtainable(d, mall):
            continue
        for slot, reals in SLOT_REAL_TYPES.items():
            if real not in reals or (slot == 5 and weapon_type is not None and real != weapon_type):
                continue
            score = _item_score(d, slot, weights) + (extra(d, slot, weights) if extra else 0.0)
            if slot not in best or score > best[slot][0]:
                best[slot] = (score, d)
    use_shield = (job == 1) if shield is None else shield
    items = []
    for slot, (_, d) in sorted(best.items()):
        if slot == 6 and (not use_shield or 5 in best and fm.real_type(best[5][1].item_type) not in ONE_HANDED):
            continue
        craft = b""
        if d.craftable:
            craft = fm.craft_name(enchant=enchant if slot == 5 and enchant else armour + 50 if slot in (0, 1, 2, 3, 4, 6) and armour else 0)
        items.append(fm.ItemUnit(d.item_type, d.type_id, slot, quality=d.quality, craft_name=craft))
    return items


# --- crafted stats and lapis (part 3's builds) -----------------------------------------------------------------------------------------

CRAFT_KEYS = STAT_NAMES + ["hp", "mp", "sp"]  # the nine crafted numbers in craft-name order (`stats::craft_values`)
# What each class wants its crafted numbers and lapis on, first first (`craftorder=`, `lapis=G:...` override it).
DEFAULT_WANTS = {0: ["hp", "str", "dex", "sp", "rec"], 1: ["hp", "rec", "str", "dex", "sp"], 2: ["hp", "str", "dex", "sp", "luc"],
                 3: ["hp", "str", "dex", "sp", "luc"], 4: ["hp", "int", "mp", "wis", "rec"], 5: ["hp", "wis", "int", "mp", "rec"]}


def craft_count(d: fm.ItemDef) -> int:
    """How many crafted numbers a drop of this item gets (`lapis::roll_craft`): weapons and rings, amulets, bracelets choose among the
    six stats (`server` 1 to 5 that many, else all six), other items among all nine (1 to 9; above 9 none)."""
    if not d.craftable:
        return 0
    real = fm.real_type(d.item_type)
    if 1 <= real <= 15 or real in (22, 23, 25):
        return d.server if 1 <= d.server <= 5 else 6
    return d.server if 1 <= d.server <= 9 else 0


def craft_size(top: int, how: str) -> int:
    """One crafted number, the smallest of three draws of 0 to `top` (`lapis::roll_craft`): `exp` its mean, rounded; `good` the size a
    tenth of the numbers reach or pass; `top` the highest."""
    n = top + 1
    if how == "top":
        return top
    if how == "good":
        return max(k for k in range(n) if ((n - k) / n) ** 3 >= 0.1)
    return round(sum(((n - k) / n) ** 3 for k in range(1, n)))


def crafted(d: fm.ItemDef, how: str, wants: list) -> tuple:
    """The nine crafted numbers of a kept drop: its count of numbers on the wanted stats in order (a player keeps the drop whose
    numbers sit where they want them), each of size `how` (`craft_size`)."""
    count = craft_count(d)
    if not count or how in ("", "0", "none"):
        return (0,) * 9
    real = fm.real_type(d.item_type)
    allowed = STAT_NAMES if (1 <= real <= 15 or real in (22, 23, 25)) else CRAFT_KEYS
    order = [k for k in wants if k in allowed] + [k for k in allowed if k not in wants]
    size = craft_size(d.craft_max, how)
    values = [0] * 9
    for k in order[:count]:
        values[CRAFT_KEYS.index(k)] = size
    return tuple(values)


def lapis_fits(lap: fm.ItemDef, item: fm.ItemDef) -> bool:
    """`lapis::fits`: the lapis' placement columns by the item's real type; never the cloak."""
    real = fm.real_type(item.item_type)
    if 1 <= real <= 15:
        return lap.level != 0
    return {16: lap.country != 0, 17: lap.jobs[0], 18: lap.jobs[1], 19: lap.jobs[2], 20: lap.jobs[3], 21: lap.jobs[4], 22: lap.jobs[5],
            23: lap.grow != 0, 25: lap.req_str != 0}.get(real, False)


def lapis_worth(lap: fm.ItemDef, wants: list) -> float:
    """A lapis' worth to a build in stat points (a stat 1, HP/MP/SP per 5, attack per 1.3, defence 1, magic defence 0.5), the wanted
    numbers weighted 1, 0.75, 0.5, 0.35, 0.25 in order and the attack of a weapon lapis always at 1."""
    numbers = dict(zip(STAT_NAMES, lap.const_stats))
    numbers.update(hp=lap.const_health[0] / 5, mp=lap.const_health[1] / 5, sp=lap.const_health[2] / 5,
                   attack=(lap.effects[0] + lap.effects[1] / 2) / 1.3, defence=lap.effects[2], magic_defence=lap.effects[3] / 2)
    weights = [1, 0.75, 0.5, 0.35, 0.25]
    worth = numbers["attack"]
    for i, k in enumerate(w for w in wants if w != "attack"):
        worth += numbers.get(k, 0) * (weights[i] if i < len(weights) else 0.1)
    return worth


@lru_cache(maxsize=None)
def _linkable_lapis() -> tuple:
    """The lapis a player can get and link without risk: obtainable (`obtainable`), not dangerous, named."""
    return tuple(d for d in data.items().by_key.values()
                 if d.item_type == fm.LAPIS_TYPE and not d.reqvg and obtainable(d) and not d.name.startswith(("???", "...")))


def pick_lapis(item: fm.ItemDef, top_grade: int, wants: list) -> int:
    """The lapis a player links into each room of this item: obtainable, not dangerous, of grade 31 to `top_grade`, the most worth
    (`lapis_worth`). One kind for every room."""
    best = (0.0, 0)
    for d in _linkable_lapis():
        if not 31 <= d.reqig <= top_grade or not lapis_fits(d, item):
            continue
        w = lapis_worth(d, wants)
        if w > best[0]:
            best = (w, d.type_id)
    return best[1]


def parse_build(spec: str) -> Build:
    raw = spec.split()
    job = family = grow = level = None
    opts = {}
    extra_items = []
    for w in raw:
        if "=" in w:
            k, v = w.split("=", 1)
            if k == "item":
                extra_items.append(v)
            else:
                opts[k] = v
        elif w.lower() in LIGHT:
            job, side = LIGHT.index(w.lower()), "light"
        elif w.lower() in FURY:
            job, side = FURY.index(w.lower()), "fury"
        elif w.lower() in RACES:
            family = RACES.index(w.lower())
        elif w.lower() in MODES:
            grow = MODES.index(w.lower())
        elif w.isdigit():
            level = int(w)
    if "job" in opts:
        job, side = int(opts["job"]), "light"
    if job is None:
        raise SystemExit(f"no class in {spec!r}")
    if family is None:
        family = DEFAULT_RACE[(side, job)]
    if job not in fm.RACE_CLASSES[family]:
        raise SystemExit(f"{RACES[family]} cannot be a {class_name(job, family)}")
    grow = 3 if grow is None else grow
    level = level or fm.max_level(grow)
    shares = DEFAULT_ALLOC[job]
    if "alloc" in opts:
        shares = {k: float(v) for k, v in (p.split(":") for p in opts["alloc"].split(","))}
    stats = [int(v) for v in opts["stats"].split(",")] if "stats" in opts else alloc_stats(job, family, grow, level, shares)
    items = []
    wants = opts["craftorder"].split(",") if "craftorder" in opts else DEFAULT_WANTS[job]
    lapis_grade, _, lapis_wants = opts.get("lapis", "").partition(":")
    lapis_wants = lapis_wants.split(",") if lapis_wants else wants

    def extra(d, slot, weights):
        """What the gear options add to an item: the lapis in its rooms and its crafted numbers, scored as `_item_score` does."""
        out = 0.0
        if lapis_grade and slot != fm.CLOAK_SLOT and d.room:
            gem = pick_lapis(d, int(lapis_grade), lapis_wants)
            if gem:
                out += min(d.room, 6) * _item_score(data.items().get(fm.LAPIS_TYPE, gem), 6, weights)
        if opts.get("craft") and d.craftable:
            v = crafted(d, opts["craft"], wants)
            out += sum(w * x for w, x in zip(weights, v[:6])) + v[6] * fm.CRAFT_HEALTH / 5.0 + (v[7] + v[8]) * fm.CRAFT_HEALTH / 20.0
        return out

    if opts.get("gear", "auto") == "auto":
        items = auto_gear(job, family, grow, level, shares, int(opts["weapon"]) if "weapon" in opts else None,
                          int(opts.get("enchant", 0)), int(opts.get("armour", 0)),
                          {"1": True, "0": False}.get(opts.get("shield", ""), None), opts.get("mall") == "1",
                          extra if (lapis_grade or opts.get("craft")) else None)
    explicit = {}  # slot -> (enchant step or None, crafted numbers or None)
    for v in extra_items:
        slot, rest = v.split(":")
        rest, _, craft = rest.partition("#")
        rest, _, step = rest.partition("@")
        key, *gems = rest.split("+")
        t, tid = (int(x) for x in key.split("/"))
        d = data.items().get(t, tid)
        if d is None:
            raise SystemExit(f"no item {t}/{tid}")
        gem_ids = tuple(int(g.split("/")[1]) for g in gems) + (0,) * (6 - len(gems))
        items = [it for it in items if it.slot != int(slot)]
        items.append(fm.ItemUnit(t, tid, int(slot), quality=d.quality, gems=gem_ids[:6],
                                 craft_name=fm.craft_name() if d.craftable else b""))
        explicit[int(slot)] = (int(step) if step else None, tuple(int(x) for x in craft.split(",")) if craft else None)
    # Crafted numbers (`craft=exp|good|top`, on `craftorder=` or the class's wants), lapis (`lapis=G` or `lapis=G:want,want`: every
    # empty room of every item but the cloak, `pick_lapis`) and enchant steps (`@n` on an item, else `enchant=` / `armour=`).
    defs = data.items()
    # `element=1..4`: a basic element lapis (Lv1, grade 34) in the weapon's first room (unless the weapon has an element of its own,
    # which a different lapis would cancel, `lapis::element`) and in the upper armour's (any element there cuts every monster blow).
    # `element=w,a` gives the weapon and the armour different ones; 0 for none.
    if opts.get("element"):
        we, _, ae = opts["element"].partition(",")
        we, ae = int(we), int(ae or we)
        for it in items:
            d = defs.get(it.item_type, it.type_id)
            gem = 66 + we if it.slot == 5 and we and not d.attrib else 145 + ae if it.slot == 1 and ae else 0
            if gem and d.room and not any(it.gems):
                it.gems = (gem,) + (0,) * 5
    for i, it in enumerate(items):
        d = defs.get(it.item_type, it.type_id)
        step, values = explicit.get(it.slot, (None, None))
        if d.craftable and ("craft" in opts or step is not None or values is not None or it.slot not in explicit):
            if step is None:
                step = int(opts.get("enchant", 0)) if it.slot == 5 else int(opts.get("armour", 0)) if it.slot in (0, 1, 2, 3, 4, 6) else 0
            if values is None:
                values = crafted(d, opts.get("craft", ""), wants)
            it.craft_name = fm.craft_name(values, step + 50 if step and it.slot != 5 else step)
        rooms = min(d.room, 6)
        if lapis_grade and it.slot != fm.CLOAK_SLOT and rooms and sum(1 for g in it.gems if g) < rooms:
            gem = pick_lapis(d, int(lapis_grade), lapis_wants)
            held = [g for g in it.gems if g]
            it.gems = tuple(held + [gem] * (rooms - len(held)) + [0] * (6 - rooms))
        items[i] = it
    learnt = auto_skills(job, family, grow, level) if opts.get("skills", "auto") == "auto" else (
        [] if opts.get("skills") == "none" else [tuple(int(x) for x in p.split("/")) for p in opts["skills"].split(",")])
    if opts.get("without"):
        dropped = {int(x) for x in opts["without"].split(",")}
        learnt = [x for x in learnt if x[0] not in dropped]
    buffs = [tuple(int(x) for x in p.split("/")) for p in opts["buffs"].split(",")] if opts.get("buffs") else []
    label = f"{class_name(job, family)} {RACES[family]} {MODES[grow]} {level}"
    return Build(job, family, grow, level, stats, items, learnt, buffs, int(opts.get("grade", 0)), label)


# --- a character's numbers -----------------------------------------------------------------------------------------------------------


@dataclass
class Character:
    build: Build
    totals: fm.Totals
    gear: fm.Gear
    fighter: fm.Fighter
    passives: fm.Passives
    interval_ms: int
    defence_element: int
    attack_skills: list  # SkillDef usable with the worn weapon

    @property
    def max_hp(self):
        return self.totals.max_health[fm.HP]

    def health(self):
        """HP, SP, MP (the order of the damage arrays)."""
        h = self.totals.max_health
        return (h[fm.HP], h[fm.SP], h[fm.MP])


def evaluate(b: Build) -> Character:
    """`server::totals`, `server::gear`, `combat::player`: the character as the server computes it."""
    sk = data.skills()
    learnt = [sk[x] for x in b.learnt if x in sk]
    applied = [sk[x] for x in b.buffs if x in sk]
    p = fm.with_buffs(fm.passives(learnt), fm.buff_bonuses(applied))
    grade = data.grades()[faction_of(b.family)][min(b.grade, 30)][1]
    base = fm.Base(b.job, b.level, b.grow, b.family, tuple(b.stats), tuple(grade))
    totals = fm.compute(data.status(), base, b.items, data.items(), p)
    g = fm.with_grade(fm.gear(b.items, data.items(), p), grade)
    for a in applied:
        if a.type_effect == fm.BUFF and a.kind == 94 and a.element < 9:
            g.element = a.element
        if a.type_effect == fm.BUFF and a.kind == 95:
            g.armour_element = a.element
    f = fm.player(b.level, totals.stats, g)
    weapon = next((it for it in b.items if it.slot == 5), None)
    real = fm.real_type(weapon.item_type) if weapon else 0
    has_shield = any(it.slot == 6 for it in b.items)
    usable = []
    for s in learnt:
        if s.type_effect not in (fm.ATTACK, fm.DEBUFF) or s.target_type not in (3, 6, 7):
            continue
        if s.kind == fm.DESPERATE:
            continue
        ok = has_shield if s.need_shield else (real >= 1 and s.need_weapons[real - 1])
        if ok:
            usable.append(s)
    speed, _ = totals.speeds()
    return Character(b, totals, g, f, p, fm.attack_interval_ms(speed), g.armour_element, usable)


# --- fights --------------------------------------------------------------------------------------------------------------------------


@dataclass
class Action:
    name: str
    skill: fm.SkillDef | None
    damage: float  # expected HP damage of one use at once (all its hits and the additional damage)
    time_ms: int  # the attack timer it takes: cast + one interval
    tick: float = 0.0  # expected damage over time per 3 s (a debuff of wType 61 that landed)
    ticks: int = 0  # its payments: keep time / 3
    of_current: float = 0.0  # damage type 5 (a share of the target's current HP): `damage` / the target's full HP, applied to what is left


def _skill_hits(s: fm.SkillDef) -> int:
    return max(s.multi_attack, 1) if s.kind == fm.MULTI_HIT else 1


def _dot(s: fm.SkillDef) -> tuple[float, int]:
    """A damage-over-time debuff (`wType` 61, `CMob::TimeDamage`, `CUser::ChkKeepSkill`): its HP damage per 3 s (type 0 values; the
    percentage types 3 to 5 are not modelled) and its payments over the keep time (every 3 s, the rest at the end)."""
    if s.type_effect == fm.DEBUFF and s.kind == 61 and s.time_damage_type == 0 and s.time_damage[0]:
        return float(s.time_damage[0]), max(s.keep_time // 3, 1)
    return 0.0, 0


def actions_vs_mob(c: Character, d: fm.MobDef, close=False) -> list:
    m = fm.monster(d)
    out = []
    if c.gear.kind is not None:
        e = ex.pve_attack(c.fighter, m, c.gear.kind, close and not c.gear.cross_road)
        out.append(Action("normal attack", None, e.per_attempt, c.interval_ms))
    for s in c.attack_skills:
        e = ex.pve_skill(c.fighter, m, d, s, c.health(), close=close and not c.gear.cross_road)
        tick, ticks = _dot(s)
        damage = e.per_attempt * _skill_hits(s) + e.hit * s.add_damage[0]
        out.append(Action(s.name or f"skill {s.id}/{s.level}", s, damage, s.ready_time * 250 + c.interval_ms, e.hit * tick, ticks,
                          damage / d.hp if s.damage_type == 5 and d.hp else 0.0))
    return out


def actions_vs_player(a: Character, t: Character, close=False) -> list:
    out = []
    absorption = t.gear.absorption
    if a.gear.kind is not None:
        e = ex.pvp_attack(a.fighter, t.fighter, a.gear.kind, t.defence_element, close, a.gear.cross_road, absorption)
        out.append(Action("normal attack", None, e.per_attempt, a.interval_ms))
    for s in a.attack_skills:
        h = fm.PvpSkillHit(a.fighter, t.fighter, t.defence_element, a.health(), t.health(), t.health(), close, a.gear.cross_road)
        e = ex.pvp_skill(h, s, absorption)
        tick, ticks = _dot(s)
        # Damage over time on a player is not absorbed (`CUser::DamageByKeepSkill`).
        damage = e.per_attempt * _skill_hits(s) + e.hit * fm.absorb(s.add_damage[0], absorption)[0]
        out.append(Action(s.name or f"skill {s.id}/{s.level}", s, damage, s.ready_time * 250 + a.interval_ms, e.hit * tick, ticks,
                          damage / t.max_hp if s.damage_type == 5 and t.max_hp else 0.0))
    return out


@dataclass
class Rotation:
    time_s: float  # time of the killing hit or tick (inf when it never comes within the limit)
    damage: float
    uses: dict
    dps: float
    sp: int = 0  # SP and MP left at the end
    mp: int = 0


DOT_MS = 3000


def rotate(c: Character, actions: list, hp: float, limit_s: float = 600.0, unlimited: bool = False, pool=None) -> Rotation:
    """Expected-value fight: at each turn of the attack timer, the ready, affordable action with the most damage per millisecond of
    timer it takes (a skill's cooldown starts when it goes off, after its cast; SP and MP are not regained while fighting, as
    `REC_FIGHTING` gives only the faction grade's fight recovery, which is counted; `unlimited` stands for potions: costs are not
    counted). A damage-over-time debuff pays every 3 s from when it lands; using it again while it runs restarts it and the
    payments not yet made are lost (`CMob::AddApplySkill`), so it is valued by the payments it adds. Stops at `hp` damage or
    `limit_s`; the kill time is that of the blow or payment that reached `hp`. A share of the current HP (damage type 5) is taken of
    what is left of `hp`. `pool` starts with that SP and MP instead of the maxima."""
    t = 0
    sp, mp = pool if pool is not None else (c.totals.max_health[fm.SP], c.totals.max_health[fm.MP])
    reduction = c.passives.bonuses.cost_reduction
    ready, last_used, uses = {}, {}, {}
    dots = {}  # skill id -> (next payment time, payments left, amount)
    total = 0.0
    regen_sp, regen_mp = c.totals.grade_fight_recovery[fm.SP], c.totals.grade_fight_recovery[fm.MP]
    limit = limit_s * 1000

    def pay_until(until):
        """Damage-over-time payments due up to `until`; the time of the one that reached `hp`, if any."""
        nonlocal total
        while dots:
            sid, (when, left, amount) = min(dots.items(), key=lambda kv: kv[1][0])
            if when > until:
                return None
            total += amount
            dots[sid] = (when + DOT_MS, left - 1, amount)
            if left - 1 <= 0:
                del dots[sid]
            if total >= hp:
                return when
        return None

    while total < hp and t < limit:
        killed = pay_until(t)
        if killed is not None:
            return Rotation(killed / 1000.0, total, uses, total / max(killed / 1000.0, 1e-3), sp, mp)
        best = last = None
        for a in actions:
            s = a.skill
            if s is not None:
                if ready.get(s.id, 0) > t:
                    continue
                if not unlimited and (sp < fm_cost(s.need_sp, reduction[0], sp) or mp < fm_cost(s.need_mp, reduction[1], mp)):
                    continue
                if s.prev_skill and not (s.prev_skill in last_used and ready.get(s.prev_skill, 0) > t):
                    continue
            value = a.damage if not a.of_current or hp == float("inf") else a.of_current * max(hp - total, 0.0)
            if a.ticks:
                running = dots.get(a.skill.id, (0, 0, 0))[1]
                value += a.tick * (a.ticks - running)
            rate = value / a.time_ms
            # A skill that costs the whole SP or MP pool (a cost of 1) is kept for when no other skill is ready and affordable.
            if s is not None and 1 in (s.need_sp, s.need_mp) and not unlimited:
                if value > 0 and (last is None or rate > last[0]):
                    last = (rate, a)
                continue
            if value > 0 and (best is None or rate > best[0]):
                best = (rate, a)
        if last is not None and (best is None or best[1].skill is None):
            best = last if best is None or last[0] > best[0] else best
        if best is None:
            # Nothing usable now: wait for the next payment or cooldown.
            nxt = [w for w, _, _ in dots.values()] + [r for r in ready.values() if r > t]
            if not nxt:
                break
            t = max(min(nxt), t + 1)
            continue
        a = best[1]
        s = a.skill
        hit_at = t + (s.ready_time * 250 if s else 0)
        if s is not None:
            sp -= fm_cost(s.need_sp, reduction[0], sp)
            mp -= fm_cost(s.need_mp, reduction[1], mp)
            ready[s.id] = hit_at + s.reset_time * 1000
            last_used[s.id] = hit_at
        killed = pay_until(hit_at)
        if killed is not None:
            return Rotation(killed / 1000.0, total, uses, total / max(killed / 1000.0, 1e-3), sp, mp)
        total += a.damage if not a.of_current or hp == float("inf") else a.of_current * max(hp - total, 0.0)
        uses[a.name] = uses.get(a.name, 0) + 1
        if a.ticks:
            dots[s.id] = (hit_at + DOT_MS, a.ticks, a.tick)
        if total >= hp:
            return Rotation(hit_at / 1000.0, total, uses, total / max(hit_at / 1000.0, a.time_ms / 1000.0), sp, mp)
        before = t
        t += a.time_ms
        ticks = t // fm.RECOVER_EVERY_MS - before // fm.RECOVER_EVERY_MS
        sp = min(sp + regen_sp * ticks, c.totals.max_health[fm.SP])
        mp = min(mp + regen_mp * ticks, c.totals.max_health[fm.MP])
    killed = pay_until(limit)
    if killed is not None:
        return Rotation(killed / 1000.0, total, uses, total / max(killed / 1000.0, 1e-3), sp, mp)
    return Rotation(float("inf"), total, uses, total / max(t / 1000.0, 1e-3), sp, mp)


def fm_cost(need: int, reduction: int, have: int) -> int:
    """`skills::spend` for one pool: a cost of exactly 1 is everything there is."""
    c = fm.i32(need - fm.i32(need / 100.0 * reduction)) if reduction else need
    return have if c == 1 else max(c, 0)


def mob_offence(d: fm.MobDef, c: Character, seconds: float = 120.0) -> dict:
    """A monster's expected damage per second on the character (`CMob::AttackAI`: entries 2 down to 0, each skipped while not ready
    or by its `byOK`; at full HP the HP-conditioned entries 3, 4, 5 and the targeted 6 are not used; a skill entry waits speed +
    2500 ms after, a normal one entry 0's time). Absorption and the upper armour's element count; blocks and reflection do not."""
    m = fm.monster(d)
    sk = data.skills()
    absorption = c.gear.absorption
    infos = []
    for e in d.attacks:
        s = sk.get((e.skill, 100)) if e.skill else None
        # `CGameData::LoadConstMob` keeps a skill entry only for attacks, debuffs and heals 41/42 (`MobDefs::attach_skills`).
        usable = s is not None and (s.type_effect in (fm.ATTACK, fm.DEBUFF) or (s.type_effect == fm.HEAL and s.kind in (41, 42)))
        infos.append(s if usable else None)
    # `take_target`: a skill entry other than the first is first ready after a random share of its reuse time (the mean here).
    ready = [s.reset_time * 500 if (i and s is not None) else 0 for i, s in enumerate(infos)]
    t, total = 0, 0.0
    uses = [0, 0, 0]
    while t < seconds * 1000:
        used = None
        for i in (2, 1, 0):
            e = d.attacks[i]
            if e.ok in (0, 3, 4, 5, 6) or t < ready[i] or (e.skill and infos[i] is None):
                continue
            used = i
            break
        if used is None:
            t += 2500
            continue
        e = d.attacks[used]
        uses[used] += 1
        s = infos[used]
        if s is not None:
            total += _mob_skill_expect(m, d, d.attacks[0], s, c)
            ready[used] = t + s.reset_time * 1000
            t += 2500
        else:
            total += ex.mob_attack(m, e.damage, e.damage_add, c.fighter, c.defence_element, absorption).per_attempt
            if used:
                ready[used] = t + e.time_ms
            t += max(d.attacks[0].time_ms, 1)
    return {"dps": total / seconds, "uses": uses}


def _mob_skill_expect(m: fm.Fighter, d: fm.MobDef, e: fm.MobAttack, s: fm.SkillDef, c: Character) -> float:
    """Expected HP damage of a monster's skill entry on the character (`combat::mob_skill_hits`, `mob_skill_damage`), by sampling the
    exact functions (a few hundred rolls; skills are a small share of a monster's damage)."""
    if s.type_effect not in (fm.ATTACK, fm.DEBUFF):
        return 0.0
    import random
    rng = random.Random(s.id)

    def rand():
        return rng.randrange(32768)

    hp, sp, mp = c.health()
    total = 0.0
    n = 300
    for _ in range(n):
        if not fm.mob_skill_hits(m, d, c.fighter, s, rand):
            continue
        _, dmg = fm.mob_skill_damage(m, e.damage, e.damage_add, (d.hp, d.sp, d.mp), c.fighter, c.defence_element, (hp, sp, mp),
                                     (hp, sp, mp), s, rand)
        total += fm.absorb(dmg[0], c.gear.absorption)[0]
    hits = max(s.multi_attack, 1) if s.kind == fm.MULTI_HIT else 1
    return total / n * hits


def bare(c: Character) -> Character:
    """The same level with no defence, evasion, LUC or absorption: the yardstick for mitigation."""
    f = fm.Fighter(level=c.fighter.level, kinds=[fm.Numbers(hit=k.hit) for k in c.fighter.kinds], wis=0)
    return Character(c.build, c.totals, fm.Gear(), f, c.passives, c.interval_ms, 0, [])


def vs_mob(c: Character, d: fm.MobDef) -> dict:
    acts = actions_vs_mob(c, d)
    rot = rotate(c, acts, d.hp)
    rot_u = rotate(c, acts, d.hp, unlimited=True)
    off = mob_offence(d, c)
    raw = mob_offence(d, bare(c))["dps"]
    mitigation = 1 - off["dps"] / raw if raw > 0 else 0.0
    exp = fm.kill_exp(d, c.build.level)
    out = {
        "mob": {"id": d.id, "name": d.name, "level": d.level, "hp": d.hp, "defence": d.defense, "magic": d.magic, "element": d.attrib,
                "exp": exp},
        "hit_rate_normal": fm.hit_rate(c.fighter, fm.monster(d), c.gear.kind) if c.gear.kind is not None else 0,
        "actions": [{"name": a.name, "damage": round(a.damage, 1), "ms": a.time_ms,
                     "dps": round(a.damage / a.time_ms * 1000, 1), "dot": round(a.tick * a.ticks, 1)} for a in acts],
        "time_to_kill_s": round(rot.time_s, 2),
        "rotation": rot.uses,
        "fight_dps": round(rot.dps, 1),
        "time_to_kill_unlimited_s": round(rot_u.time_s, 2),
        "rotation_unlimited": rot_u.uses,
        "mob_dps": round(off["dps"], 1),
        "time_to_die_s": round(c.max_hp / off["dps"], 1) if off["dps"] > 0 else float("inf"),
        "mitigation": round(mitigation, 3),
        "effective_hp": round(c.max_hp / (1 - mitigation), 0) if mitigation < 1 else float("inf"),
        "hp_lost_per_kill": round(off["dps"] * rot.time_s, 0) if rot.time_s != float("inf") else None,
        "exp_per_fight_minute": round(exp / rot.time_s * 60, 0) if rot.time_s not in (0, float("inf")) else None,
    }
    return out


def vs_player(a: Character, b: Character, close=False, unlimited=False) -> dict:
    ab = rotate(a, actions_vs_player(a, b, close), b.max_hp, unlimited=unlimited)
    ba = rotate(b, actions_vs_player(b, a, close), a.max_hp, unlimited=unlimited)
    return {
        "a": a.build.label, "b": b.build.label,
        "a_hits_b": {"time_to_kill_s": round(ab.time_s, 2), "dps": round(ab.dps, 1), "rotation": ab.uses,
                     "actions": [{"name": x.name, "damage": round(x.damage, 1), "ms": x.time_ms} for x in actions_vs_player(a, b, close)]},
        "b_hits_a": {"time_to_kill_s": round(ba.time_s, 2), "dps": round(ba.dps, 1), "rotation": ba.uses,
                     "actions": [{"name": x.name, "damage": round(x.damage, 1), "ms": x.time_ms} for x in actions_vs_player(b, a, close)]},
        "winner": a.build.label if ab.time_s < ba.time_s else b.build.label if ba.time_s < ab.time_s else "even",
        # Both kills on the same turn of the timer: the share of the other's HP each had taken by then (above 1: overkill).
        "share_at_first_kill": [round(min(ab.damage, rotate(a, actions_vs_player(a, b, close), float("inf"),
                                                              limit_s=min(ab.time_s, ba.time_s), unlimited=unlimited).damage) / b.max_hp, 2),
                                round(min(ba.damage, rotate(b, actions_vs_player(b, a, close), float("inf"),
                                                            limit_s=min(ab.time_s, ba.time_s), unlimited=unlimited).damage) / a.max_hp, 2)],
    }


# --- reports -------------------------------------------------------------------------------------------------------------------------


def sheet(c: Character) -> dict:
    b, t, f, g = c.build, c.totals, c.fighter, c.gear
    names = data.items()
    learnt = b.learnt
    return {
        "build": b.label, "faction": ["light", "fury"][faction_of(b.family)], "grade": b.grade,
        "base_stats": dict(zip(STAT_NAMES, b.stats)), "stats": dict(zip(STAT_NAMES, t.stats)),
        "hp": t.max_health[fm.HP], "mp": t.max_health[fm.MP], "sp": t.max_health[fm.SP],
        "attack_speed": t.speeds()[0], "interval_ms": c.interval_ms, "move_speed": t.speeds()[1],
        "melee": vars(f.kinds[fm.MELEE]), "ranged": vars(f.kinds[fm.RANGED]), "magic": vars(f.kinds[fm.MAGIC]),
        "critical": f.critical, "attack_spread": g.attack_var, "item_attack": g.attack, "absorption": g.absorption,
        "attack_element": g.element, "defence_element": g.armour_element, "range": g.range,
        "weapon_kind": ["melee", "ranged"][g.kind] if g.kind is not None else None,
        "items": [f"{it.slot}: {it.item_type}/{it.type_id} {names.get(it.item_type, it.type_id).name}" for it in sorted(b.items, key=lambda i: i.slot)],
        "skills_learnt": len(learnt), "skill_points_used": skill_points_used(learnt),
        "skill_points_available": fm.reset_skill_points(b.grow, b.level),
        "attack_skills": [s.name for s in c.attack_skills],
    }


def print_dict(d, indent=0):
    pad = "  " * indent
    for k, v in d.items():
        if isinstance(v, dict):
            print(f"{pad}{k}:")
            print_dict(v, indent + 1)
        elif isinstance(v, list) and v and isinstance(v[0], dict):
            print(f"{pad}{k}:")
            for row in v:
                print(f"{pad}  - " + ", ".join(f"{a}={b}" for a, b in row.items()))
        elif isinstance(v, list):
            print(f"{pad}{k}: " + ("; ".join(map(str, v)) if v else "-"))
        else:
            print(f"{pad}{k}: {v}")


def exp_curve(grow: int) -> list:
    """Per level: experience to pass it, the running total, and the points it gives (`combat::add_exp`)."""
    t = data.exp_table()
    rows = []
    for level in range(1, fm.max_level(grow) + 1):
        need = t.total(grow, level) - t.total(grow, level - 1)
        rows.append({"level": level, "exp": need, "total": t.total(grow, level),
                     "stat_points": fm.reset_stats(0, 0, grow, level)[0], "skill_points": fm.reset_skill_points(grow, level)})
    return rows


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("what", choices=["char", "mob", "pvp", "curve"])
    ap.add_argument("builds", nargs="*")
    ap.add_argument("--mob", type=int, action="append", help="monster id (repeat)")
    ap.add_argument("--mode", default="ultimate")
    ap.add_argument("--close", action="store_true", help="PvP within 3 (ranged attacks lose 30 %%)")
    ap.add_argument("--unlimited", action="store_true", help="PvP: SP and MP never run out (potions)")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--refresh", action="store_true", help="read the database again")
    args = ap.parse_args(argv)
    if args.refresh:
        data.refresh()
    if args.what == "curve":
        out = exp_curve(MODES.index(args.mode))
    else:
        chars = [evaluate(parse_build(s)) for s in args.builds]
        if args.what == "char":
            out = [sheet(c) for c in chars]
        elif args.what == "mob":
            out = [{"build": c.build.label, **vs_mob(c, data.mobs()[m])} for c in chars for m in (args.mob or [])]
        else:
            out = vs_player(chars[0], chars[1], args.close, args.unlimited)
    if args.json:
        json.dump(out, sys.stdout, indent=1, ensure_ascii=False, default=str)
        print()
    else:
        for row in out if isinstance(out, list) else [out]:
            print_dict(row)
            print()


if __name__ == "__main__":
    main()
