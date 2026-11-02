#!/usr/bin/env python3
"""Part 3 of the balance analysis (docs/plans/balance/3-classes.md): builds a player would make for each of the twelve classes at
levels 15, 30 and 60 (and 55 for levelling through the 51-58 fields), how they fight the yardstick monsters and each other, the
faction pairs, the modes and a few levers. Writes CSV tables to data/balance/ (git-ignored) and prints the document's tables.

    python3 builds.py                 # every stage (about half an hour with --jobs 4)
    python3 builds.py --only final,pvp,factions,modes,levers
    python3 builds.py --report        # the Markdown tables from the CSVs

How a build is made (each step is a function below; the document explains the choices):
  1. weapon: of every weapon kind the class may wield (with a shield where one-handed and the class takes one), the picker's best
     obtainable item of that kind; kept: the kind whose reference build does best (`choose_weapon`).
  2. skills: the passives for that weapon, the class's lasting buffs and opening buffs, then the attack skills by the damage they do
     in the reference fights, the level-60 percentage skill, then defensive and debuff skills, until the skill points run out
     (`choose_skills`).
  3. stats and lapis: every split of the free points in quarters over four stats, and lapis by stat or by HP first; kept: the best on
     PvE (experience per hour against the level's field monster) and PvP (against the twelve classes) together (`search`), twice (the
     second round against the first round's choices).
  4. gear: enchant weapon +3, armour +4 (what play reaches, part 2 section 4.3); crafted numbers a kept drop has (`craft=exp`);
     grade-35 lapis (Lv4 and Dual, 9 to 12 % a link); an element lapis in the weapon and the upper armour.
Every build is a model.py string (`builds.csv`), so `model.py char|mob|pvp "<string>"` reruns it.
"""
from __future__ import annotations

import argparse
import csv
import itertools
import math
import statistics
import sys
from dataclasses import replace
from functools import lru_cache
from multiprocessing import Pool

import data
import formulas as fm
import model

OUT = data.ROOT / "data" / "balance"
CLASSES = model.LIGHT + model.FURY
PAIRS = list(zip(model.LIGHT, model.FURY))
LEVELS = (15, 30, 60)
PVE_LEVELS = (15, 30, 55, 60)
# The monster each level is scored against: part 2's yardsticks (2125, 2183, 1547), the most placed level-55 monster of the level-50+
# fields (1501, map 45, Deep Desert 1) and of the Elemental Cave (982, level 53, maps 25 and 60).
FIELD = {15: 2125, 30: 2183, 55: 1501, 60: 1501}
MOBS = {15: [2125], 30: [2183], 55: [1501, 982], 60: [1501, 1547, 982]}
PERCENT_SKILLS = (180, 182, 183, 184)
MAGE_DOTS = (65, 66, 68, 82, 146, 321)  # the mage's and pagan's damage-over-time skills (`wType` 61)
DAMAGE_STAT = {0: "str", 1: "str", 2: "str", 3: "str", 4: "int", 5: "int"}
ALLOC_STATS = {0: ("str", "dex", "rec", "luc"), 1: ("str", "dex", "rec", "luc"), 2: ("str", "dex", "rec", "luc"),
               3: ("str", "dex", "rec", "luc"), 4: ("int", "wis", "rec", "luc"), 5: ("int", "wis", "rec", "luc")}
# The gear every build gets (part 2 sections 3.5, 4.1, 4.3): what a player reaches in play.
GEAR = {"expected": "enchant=3 armour=4 craft=exp lapis=35", "good": "enchant=3 armour=4 craft=good lapis=36",
        "plain": "enchant=0 armour=0"}
ELEMENT = 1  # in PvP everyone's weapon and armour carry the same element (factor 1.0); against a monster the strong one is used
# The class's lasting buffs (kept up: a keep time of 300 s or more and no real cooldown) and party buffs (target type 4, which reach
# the caster too): on in every fight.
LASTING = {0: [30, 31, 112, 139], 1: [30, 31, 59, 111, 304, 139], 2: [44, 120], 3: [133, 121], 4: [117, 132], 5: [123, 132]}
# Opening buffs (a short keep time, a long cooldown): cast before a PvP fight, not counted against monsters (their uptime is a fraction).
OPENING = {0: [58, 129, 128], 1: [75], 2: [91, 136, 142], 3: [136, 142], 4: [], 5: []}
# Race skills (part 1 section 6.2): Berserker is kept up (180 s / 180 s), Bloody Karma opens a fight (60 s / 180 s), Battle Cry only
# raises sitting recovery (resting between kills), Evasion costs every SP and MP point and is not cast (see `faction_rows`).
RACE_LASTING = {0: [188], 3: [190]}
RACE_OPENING = {2: [191]}
# The party's lasting buffs from the other five classes of a side (one of each): Leadership, Tactical Formation, Incantation, Marksman
# Song, Support Spell, Prayer, and the priest's Aura of Peace on the others.
PARTY = [112, 111, 120, 121, 117, 123, 116]
# Skills that matter but the model does not count (blocks, reflection, heals, stuns, debuffs' lasting effects): learnt with the points
# left, in this order.
UTILITY = {0: [73, 157, 55, 40, 302, 24, 32, 56],
           1: [90, 89, 181, 175, 131, 130, 98, 99, 306, 41, 62, 105, 25, 32],
           2: [141, 145, 63, 161, 310, 134, 101, 110, 26, 32, 114],
           3: [141, 108, 51, 33, 172, 177, 101, 110, 27, 32, 46],
           4: [74, 47, 96, 35, 95, 109, 28, 32, 49],
           5: [102, 186, 144, 148, 168, 54, 86, 103, 124, 116, 42, 85, 74, 28, 32]}
# A priest heals: these come before its attack skills.
HEALER_FIRST = {5: [102, 186]}
# Debuffs a class opens a PvP fight with that lower the target's numbers (the "opening debuffs" variant at 60).
DEBUFFS = {0: [157], 1: [98, 306], 2: [145, 101, 110], 3: [108, 101, 110], 4: [96], 5: []}


def shares_str(shares: dict) -> str:
    return ",".join(f"{k}:{v}" for k, v in shares.items() if v)


def skills_str(learnt) -> str:
    return ",".join(f"{i}/{lv}" for i, lv in learnt)


def spec(cls: str, mode: str, level: int, shares: dict, weapon: int, shield: bool, gear: str, lapis_wants: str = "",
         learnt=None, buffs=(), element=(ELEMENT, ELEMENT), extra: str = "") -> str:
    """A model.py build string."""
    parts = [cls, mode, str(level), f"alloc={shares_str(shares)}", f"weapon={weapon}", f"shield={int(shield)}", GEAR[gear]]
    if lapis_wants and "lapis=" in GEAR[gear]:
        parts[-1] = parts[-1] + f":{lapis_wants}"
    if element:
        parts.append(f"element={element[0]},{element[1]}")
    if learnt is not None:
        parts.append(f"skills={skills_str(learnt)}")
    if buffs:
        parts.append("buffs=" + ",".join(f"{i}/{lv}" for i, lv in buffs))
    if extra:
        parts.append(extra)
    return " ".join(parts)


# --- buffs ----------------------------------------------------------------------------------------------------------------------------


def learnt_buffs(learnt, ids) -> list:
    have = dict(learnt)
    return [(i, have[i]) for i in ids if i in have]


def race_buffs(family: int, grow: int, level: int, ids) -> list:
    """Race skills cost no points and every character has them (`may_learn`); their highest level allowed here."""
    sk = data.skills()
    out = []
    for i in ids:
        top = max((lv for lv in (1, 2, 3) if (i, lv) in sk and sk[(i, lv)].required_level <= level
                   and model.race_fits(sk[(i, lv)].country, family) and model.grow_fits(sk[(i, lv)].grow, grow)), default=0)
        if top:
            out.append((i, top))
    return out


def buffs_for(job: int, family: int, grow: int, level: int, learnt, pvp: bool, party: bool = False) -> list:
    buffs = learnt_buffs(learnt, LASTING[job]) + race_buffs(family, grow, level, RACE_LASTING.get(family, []))
    if pvp:
        buffs += learnt_buffs(learnt, OPENING[job]) + race_buffs(family, grow, level, RACE_OPENING.get(family, []))
    if party:
        sk = data.skills()
        have = {i for i, _ in buffs}
        for i in PARTY:
            if i in have:
                continue
            top = max((lv for lv in (1, 2, 3) if (i, lv) in sk and sk[(i, lv)].required_level <= level
                       and model.grow_fits(sk[(i, lv)].grow, grow)), default=0)
            if top:
                buffs.append((i, top))
    return buffs


# --- fights ---------------------------------------------------------------------------------------------------------------------------


def best_element(mob: fm.MobDef) -> int:
    """The basic element (1 to 4) whose weapon lapis does most against this monster (`combat::ELEMENTS`, row against its element)."""
    return max(range(1, 5), key=lambda e: fm.ELEMENTS[e][mob.attrib] if mob.attrib < 9 else 1.0)


def spent(c: model.Character, acts: list, uses: dict) -> tuple:
    """SP and MP a rotation spent (`skills::spend`; a cost of 1 is the whole pool)."""
    by = {a.name: a for a in acts}
    reduction = c.passives.bonuses.cost_reduction
    sp = mp = 0
    for name, n in uses.items():
        s = by[name].skill if name in by else None
        if s is not None:
            sp += n * model.fm_cost(s.need_sp, reduction[0], c.totals.max_health[fm.SP])
            mp += n * model.fm_cost(s.need_mp, reduction[1], c.totals.max_health[fm.MP])
    return sp, mp


BETWEEN_KILLS_S = 5.0  # walking to and taking on the next monster (inferred; spawn density is not modelled)


def rest_s(c: model.Character, hp: float, sp: float, mp: float) -> float:
    """Sitting until HP, SP and MP are back (`zone::Zones::recover`: every 5 s, 5 % of the maximum + 28 + the sit-recovery
    percentages)."""
    pct = c.passives.bonuses.sit_recovery_percent
    t = 0.0
    for lost, i in ((hp, fm.HP), (sp, fm.SP), (mp, fm.MP)):
        per_tick = fm.recovery("sitting", c.totals.max_health[i], pct[i])
        if lost > 0 and per_tick > 0:
            t = max(t, lost / per_tick * fm.RECOVER_EVERY_MS / 1000)
    return t


def mob_fight(c: model.Character, mob: fm.MobDef, full: bool = True) -> dict:
    """Kill time (SP/MP pool and unlimited), death time, rest, experience per hour (solo, no bonuses; 0 when it dies first)."""
    acts = model.actions_vs_mob(c, mob)
    rot = model.rotate(c, acts, mob.hp)
    off = model.mob_offence(mob, c, 60.0 if full else 30.0)["dps"]
    die = c.max_hp / off if off > 0 else float("inf")
    kill = rot.time_s
    exp = fm.kill_exp(mob, c.build.level)
    out = {"kill_s": kill, "die_s": die, "mob_dps": off, "exp": exp, "hit": fm.hit_rate(c.fighter, fm.monster(mob), c.gear.kind)
           if c.gear.kind is not None else 0, "rotation": rot.uses}
    if full:
        out["kill_unlimited_s"] = model.rotate(c, acts, mob.hp, unlimited=True).time_s
    if kill == float("inf") or die < kill:
        out.update(rest_s=None, exp_h=0.0, hp_lost=None)
        return out
    sp, mp = spent(c, acts, rot.uses)
    hp_lost = off * (kill + c.interval_ms / 1000)
    r = rest_s(c, hp_lost, sp, mp)
    # A kill takes its blows and the attack timer after the last, then `BETWEEN_KILLS_S` to reach the next monster, and the rest.
    out.update(rest_s=r, exp_h=exp * 3600 / (kill + c.interval_ms / 1000 + BETWEEN_KILLS_S + r), hp_lost=hp_lost, sp_spent=sp, mp_spent=mp)
    return out


PVP_LIMIT_S = 300.0
OPENING_IDS = {i for ids in list(OPENING.values()) + list(RACE_OPENING.values()) for i in ids}


def plain(c: model.Character) -> model.Character:
    """The character once its opening buffs have run out (lasting buffs and anything else applied stay)."""
    if not hasattr(c, "_plain"):
        buffs = [x for x in c.build.buffs if x[0] not in OPENING_IDS]
        c._plain = model.evaluate(replace(c.build, buffs=buffs)) if len(buffs) != len(c.build.buffs) else c
    return c._plain


def opening_keep(c: model.Character) -> float:
    sk = data.skills()
    keeps = [sk[x].keep_time for x in c.build.buffs if x[0] in OPENING_IDS and x in sk]
    return float(min(keeps)) if keeps else float("inf")


def ttk(a: model.Character, b: model.Character, close=False) -> float:
    """Seconds for `a` to kill `b` from full health, SP/MP pool, at range (`model.rotate`), in two phases: with both sides' opening
    buffs until the first of them runs out, then without them (the share of HP lost, the SP and MP left carried over; cooldowns and
    running damage over time start afresh). `PVP_LIMIT_S` when it does not kill within it (shown as "never")."""
    t_open = min(opening_keep(a), opening_keep(b), PVP_LIMIT_S)
    r1 = model.rotate(a, model.actions_vs_player(a, b, close), b.max_hp, limit_s=t_open)
    if r1.time_s <= t_open or t_open >= PVP_LIMIT_S:
        return min(r1.time_s, PVP_LIMIT_S)
    a0, b0 = plain(a), plain(b)
    left = b0.max_hp * max(1 - r1.damage / b.max_hp, 0.0)
    r2 = model.rotate(a0, model.actions_vs_player(a0, b0, close), left, limit_s=PVP_LIMIT_S - t_open, pool=(r1.sp, r1.mp))
    return min(t_open + r2.time_s, PVP_LIMIT_S)


# --- step 1: the weapon ---------------------------------------------------------------------------------------------------------------


def weapon_options(cls: str, mode: str, level: int) -> list:
    """(real type, shield) the class may wield and can get at this level."""
    b = model.parse_build(f"{cls} {mode} {level} gear=none skills=none")
    job, family, grow = b.job, b.family, b.grow
    kinds = set()
    shield_ok = False
    for d in data.items().by_key.values():
        real = fm.real_type(d.item_type)
        if real == 0 or not model.may_equip(d, job, family, grow, level) or d.name.startswith(("???", "...")) or not model.obtainable(d):
            continue
        if 1 <= real <= 15:
            kinds.add(real)
        elif real == 19:
            shield_ok = True
    out = [(k, False) for k in sorted(kinds)]
    if shield_ok:
        out += [(k, True) for k in sorted(kinds) if k in model.ONE_HANDED]
    return out


def reference_shares(job: int) -> dict:
    return {DAMAGE_STAT[job]: 3, "dex": 1} if job < 4 else {"int": 3, "wis": 1}


@lru_cache(maxsize=None)
def reference_field(mode: str, level: int, gear: str = "expected") -> tuple:
    """The twelve classes' reference builds (3/4 the damage stat, 1/4 DEX or WIS, the picker's weapon, every skill): the opponents
    while weapons and skills are chosen."""
    return tuple(model.evaluate(model.parse_build(spec(c, mode, level, reference_shares(CLASSES.index(c) % 6),
                                                       *_default_weapon(c, mode, level), gear))) for c in CLASSES)


def choose_weapon(cls: str, mode: str, level: int, gear: str = "expected") -> tuple:
    """The weapon kind whose reference build does best on PvE (experience per hour against the field monster) and PvP (the geometric
    mean over the twelve reference builds of their kill time over ours) together; returns (real type, shield, the options)."""
    mob = data.mobs()[FIELD[level]]
    field_ = reference_field(mode, level, gear) if level in LEVELS else ()
    rows = []
    for real, shield in weapon_options(cls, mode, level):
        b = model.parse_build(spec(cls, mode, level, reference_shares(CLASSES.index(cls) % 6), real, shield, gear))
        c = model.evaluate(b)
        if not c.attack_skills and c.gear.kind is None:
            continue
        f = mob_fight(c, mob, full=False)
        logs = [math.log(min(max(ttk(o, c) / max(ttk(c, o), 0.05), 0.1), 10)) for o in field_]
        rows.append({"real": real, "shield": shield, "pve": f["exp_h"] or 0.0, "die_over_kill": f["die_s"] / max(f["kill_s"], 0.1),
                     "pvp": math.exp(sum(logs) / len(logs)) if logs else 1.0,
                     "weapon": next((data.items().get(it.item_type, it.type_id).name for it in b.items if it.slot == 5), "")})
    best_pve = max((r["pve"] for r in rows), default=0) or None
    best_ratio = max(r["die_over_kill"] for r in rows)
    best_pvp = max(r["pvp"] for r in rows)
    for r in rows:
        e = r["pve"] / best_pve if best_pve else r["die_over_kill"] / best_ratio
        r["score"] = math.sqrt(max(e, 1e-6) * r["pvp"] / best_pvp)
    best = max(rows, key=lambda r: r["score"])
    return best["real"], best["shield"], rows


def _default_weapon(cls: str, mode: str, level: int) -> tuple:
    """The weapon the picker takes with the reference shares (for the opponent while choosing)."""
    b = model.parse_build(f"{cls} {mode} {level} {GEAR['plain']} skills=none")
    w = next((it for it in b.items if it.slot == 5), None)
    return (fm.real_type(w.item_type) if w else 0), any(it.slot == 6 for it in b.items)


# --- step 2: the skills ---------------------------------------------------------------------------------------------------------------


def level_cost(sid: int, top: int) -> int:
    sk = data.skills()
    return sum(sk[(sid, lv)].points for lv in range(1, top + 1) if (sid, lv) in sk)


def choose_skills(cls: str, mode: str, level: int, real: int, shield: bool, gear: str = "expected", exclude=()) -> tuple:
    """The skills a player learns within the points (`item_use::reset_skill_points`), in this order: passives that count for the
    worn weapon (and shield) and the always-useful ability passives; the class's lasting and opening buffs; for the priest its heals;
    the attack skills by their share of the damage in the reference fights (the field monster with potions, the mirror class from
    the SP/MP pool); the level-60 percentage skill; then `UTILITY`. Each at the highest level the remaining points allow. Returns
    (learnt, points left, the attack-skill shares). `exclude`: skills the player does without (the points go elsewhere)."""
    extra = f"without={','.join(map(str, exclude))}" if exclude else ""
    b = model.parse_build(spec(cls, mode, level, reference_shares(CLASSES.index(cls) % 6), real, shield, gear, extra=extra))
    job, family, grow = b.job, b.family, b.grow
    sk = data.skills()
    top = {i: lv for i, lv in model.auto_skills(job, family, grow, level) if i not in exclude}
    # Shares of the damage with every skill learnt.
    c = model.evaluate(b)
    mob = data.mobs()[FIELD[level]]
    mirror = CLASSES[(CLASSES.index(cls) + 6) % 12]
    other = model.evaluate(model.parse_build(spec(mirror, mode, level, reference_shares(job), *_default_weapon(mirror, mode, level), gear)))
    share = {}
    for acts, hp, unlimited in ((model.actions_vs_mob(c, mob), mob.hp, True), (model.actions_vs_player(c, other), other.max_hp, False)):
        rot = model.rotate(c, acts, hp * 3, limit_s=120, unlimited=unlimited)
        by = {a.name: a for a in acts}
        total = sum(by[n].damage * u + by[n].tick * by[n].ticks * u for n, u in rot.uses.items()) or 1
        for n, u in rot.uses.items():
            s = by[n].skill
            if s is not None:
                share[s.id] = share.get(s.id, 0) + (by[n].damage + by[n].tick * by[n].ticks) * u / total
    order = []
    for sid in top:
        s = sk[(sid, top[sid])]
        if s.type_effect != fm.PASSIVE:
            continue
        weapons = [w for w in s.weapons if w]
        if (s.kind in (1, 8) and real in weapons) or (s.kind == 10 and shield) or (s.kind == 4 and sid not in (3, 4)):
            order.append(sid)
    order += LASTING[job] + OPENING[job] + HEALER_FIRST.get(job, [])
    order += sorted(share, key=lambda i: -share[i])
    if level >= 60:
        order += [i for i in PERCENT_SKILLS if i in top]
    order += UTILITY[job]
    budget = fm.reset_skill_points(grow, level)
    learnt = []
    seen = set()
    for sid in order:
        if sid in seen or sid not in top:
            continue
        seen.add(sid)
        for lv in range(top[sid], 0, -1):
            cost = level_cost(sid, lv)
            if cost <= budget:
                if cost or lv:
                    learnt.append((sid, lv))
                budget -= cost
                break
    return sorted(learnt), budget, share


# --- step 3: stats and lapis ----------------------------------------------------------------------------------------------------------


def grid(job: int) -> list:
    """Every split of the free points in quarters over the class's four stats, and lapis by stat first or HP first."""
    stats = ALLOC_STATS[job]
    out = []
    for q in itertools.product(range(5), repeat=4):
        if sum(q) == 4:
            shares = {s: n for s, n in zip(stats, q) if n}
            for lapis in ("stat", "hp"):
                out.append((shares, lapis))
    return out


def lapis_wants(job: int, how: str, shares: dict) -> str:
    """The lapis order: the build's largest stat (the damage stat when tied) and HP, in the order asked, then the rest."""
    main = max(shares, key=lambda k: (shares[k], k == DAMAGE_STAT[job]))
    first = [main, "hp"] if how == "stat" else ["hp", main]
    rest = [k for k in model.DEFAULT_WANTS[job] if k not in first]
    return ",".join(first + rest)


def craft_order(job: int, shares: dict) -> str:
    """Crafted numbers: HP first, then the build's stats by share, then SP or MP."""
    stats = sorted(shares, key=lambda k: -shares[k])
    return ",".join(["hp"] + stats + (["sp"] if job < 4 else ["mp"]))


class Plan:
    """What `prepare` decided for a class and level: weapon, shield, skills."""

    def __init__(self, cls, mode, level, real, shield, learnt, left, share, weapons):
        self.cls, self.mode, self.level, self.real, self.shield = cls, mode, level, real, shield
        self.learnt, self.left, self.share, self.weapons = learnt, left, share, weapons

    @property
    def job(self):
        return CLASSES.index(self.cls) % 6

    @property
    def family(self):
        return model.parse_build(f"{self.cls} {self.mode} {self.level} gear=none skills=none").family

    @property
    def grow(self):
        return model.MODES.index(self.mode)

    def spec(self, shares, lapis, gear="expected", pvp=True, party=False, element=(ELEMENT, ELEMENT), without=(), extra_buffs=()):
        learnt = [x for x in self.learnt if x[0] not in without]
        buffs = buffs_for(self.job, self.family, self.grow, self.level, learnt, pvp, party) + list(extra_buffs)
        extra = f"craftorder={craft_order(self.job, shares)}"
        return spec(self.cls, self.mode, self.level, shares, self.real, self.shield, gear, lapis_wants(self.job, lapis, shares),
                    learnt, buffs, element, extra)


def prepare(args) -> Plan:
    cls, mode, level = args
    real, shield, weapons = choose_weapon(cls, mode, level)
    learnt, left, share = choose_skills(cls, mode, level, real, shield)
    return Plan(cls, mode, level, real, shield, learnt, left, share, weapons)


def score_candidates(args) -> list:
    """Every grid point of one class and level: PvE (experience per hour against the field monster, or the death-to-kill ratio when
    every candidate dies) and PvP (the geometric mean over the opponents of their kill time over ours, each clipped to 0.1 to 10)."""
    plan, opponents = args
    mob = data.mobs()[FIELD[plan.level]]
    opp = [model.evaluate(model.parse_build(s)) for s in opponents]
    rows = []
    for shares, lapis in grid(plan.job):
        c_pve = model.evaluate(model.parse_build(plan.spec(shares, lapis, pvp=False, element=(best_element(mob), ELEMENT))))
        f = mob_fight(c_pve, mob, full=False)
        c = model.evaluate(model.parse_build(plan.spec(shares, lapis)))
        logs = []
        for o in opp:
            mine, theirs = ttk(c, o), ttk(o, c)
            logs.append(math.log(min(max(theirs / max(mine, 0.05), 0.1), 10)))
        rows.append({"shares": shares, "lapis": lapis, "exp_h": f["exp_h"] or 0.0, "die_over_kill": f["die_s"] / max(f["kill_s"], 0.1),
                     "pvp": math.exp(sum(logs) / len(logs)), "hp": c.max_hp})
    best_pve = max(r["exp_h"] for r in rows)
    best_ratio = max(r["die_over_kill"] for r in rows)
    best_pvp = max(r["pvp"] for r in rows)
    for r in rows:
        r["pve_rel"] = r["exp_h"] / best_pve if best_pve else r["die_over_kill"] / best_ratio
        r["pvp_rel"] = r["pvp"] / best_pvp
        r["score"] = math.sqrt(max(r["pve_rel"], 1e-6) * r["pvp_rel"])
    return rows


# --- runs -----------------------------------------------------------------------------------------------------------------------------


def write_csv(name: str, rows: list) -> None:
    if not rows:
        return
    keys = list(dict.fromkeys(k for r in rows for k in r))
    with open(OUT / name, "w", newline="") as f:
        w = csv.DictWriter(f, keys)
        w.writeheader()
        w.writerows(rows)


def read_csv(name: str) -> list:
    with open(OUT / name, newline="") as f:
        return list(csv.DictReader(f))


def fmt_s(x) -> str:
    if x is None or x == "":
        return "-"
    x = float(x)
    if x == float("inf") or x >= 300:
        return "never"
    return f"{x:.1f}" if x < 10 else f"{x:.0f}"


def sheet_row(c: model.Character) -> dict:
    f, g, t = c.fighter, c.gear, c.totals
    kind = g.kind if g.kind is not None else fm.MAGIC
    main = fm.MAGIC if c.build.job >= 4 else kind
    k = f.kinds[main]
    return {"hp": t.max_health[fm.HP], "sp": t.max_health[fm.SP], "mp": t.max_health[fm.MP],
            **{s: v for s, v in zip(model.STAT_NAMES, t.stats)}, "attack": k.attack, "spread": g.attack_var, "hit": k.hit,
            "evasion": f.kinds[fm.MELEE].evasion, "defence": f.kinds[fm.MELEE].defence, "ranged_defence": f.kinds[fm.RANGED].defence,
            "magic_defence": f.kinds[fm.MAGIC].defence, "critical": f.critical, "interval_ms": c.interval_ms, "absorption": g.absorption,
            "weapon": next((data.items().get(it.item_type, it.type_id).name for it in c.build.items if it.slot == 5), ""),
            "skill_points": model.skill_points_used(c.build.learnt), "skill_points_max": fm.reset_skill_points(c.build.grow, c.build.level)}




# --- build strings --------------------------------------------------------------------------------------------------------------------


def get_opt(s: str, key: str) -> str:
    return next((t.split("=", 1)[1] for t in s.split() if t.startswith(key + "=")), "")


def set_opt(s: str, key: str, value: str) -> str:
    """The build string with `key=value` (replaced, or added at the end)."""
    tokens = [t for t in s.split() if not t.startswith(key + "=")]
    return " ".join(tokens + [f"{key}={value}"])


def drop_opt(s: str, *keys: str) -> str:
    return " ".join(t for t in s.split() if not t.startswith(tuple(k + "=" for k in keys)))


def add_buffs(s: str, extra) -> str:
    have = [tuple(int(x) for x in t.split("/")) for t in get_opt(s, "buffs").split(",") if t]
    ids = {i for i, _ in have}
    merged = have + [x for x in extra if x[0] not in ids]
    return set_opt(s, "buffs", ",".join(f"{i}/{lv}" for i, lv in merged))


def gear_variant(s: str, gear: str) -> str:
    """The same build with other gear: `good` (crafted numbers at the tenth-best size, grade-36 lapis) or `plain` (no enchant, no
    crafted numbers, no lapis, no element)."""
    if gear == "good":
        return set_opt(set_opt(s, "craft", "good"), "lapis", "36:" + get_opt(s, "lapis").partition(":")[2])
    if gear == "plain":
        return set_opt(set_opt(drop_opt(s, "craft", "lapis", "element"), "enchant", "0"), "armour", "0")
    return s


# --- stages ---------------------------------------------------------------------------------------------------------------------------


def save_plans(plans: list) -> None:
    write_csv("builds_plans.csv", [{"class": p.cls, "mode": p.mode, "level": p.level, "real": p.real, "shield": int(p.shield),
                                    "skills": skills_str(p.learnt), "points_left": p.left,
                                    "damage_shares": " ".join(f"{i}:{v:.2f}" for i, v in sorted(p.share.items(), key=lambda kv: -kv[1])),
                                    "weapon_options": " | ".join(f"{w['real']}{'+shield' if w['shield'] else ''} {w['weapon']} "
                                                                 f"score {w['score']:.2f}" for w in p.weapons)} for p in plans])


def load_plans() -> dict:
    out = {}
    for r in read_csv("builds_plans.csv"):
        learnt = [tuple(int(x) for x in t.split("/")) for t in r["skills"].split(",") if t]
        share = {int(k): float(v) for k, v in (t.split(":") for t in r["damage_shares"].split())}
        out[(r["class"], r["mode"], int(r["level"]))] = Plan(r["class"], r["mode"], int(r["level"]), int(r["real"]), r["shield"] == "1",
                                                             learnt, int(r["points_left"]), share, [])
    return out


def stage_plans(pool) -> None:
    keys = ([(c, "ultimate", lv) for lv in PVE_LEVELS for c in CLASSES]
            + [(c, m, lv) for m in ("normal", "hard") for lv in LEVELS for c in CLASSES])
    save_plans(pool.map(prepare, keys, chunksize=1))


def stage_search(pool) -> None:
    plans = load_plans()
    rows_out, chosen = [], {}
    for level in LEVELS:
        ps = [plans[(c, "ultimate", level)] for c in CLASSES]
        opponents = [p.spec(reference_shares(p.job), "stat") for p in ps]
        for rnd in (1, 2):
            results = pool.map(score_candidates, [(p, opponents) for p in ps], chunksize=1)
            for p, rows in zip(ps, results):
                for r in rows:
                    rows_out.append({"class": p.cls, "level": level, "round": rnd, "alloc": shares_str(r["shares"]), "lapis": r["lapis"],
                                     "exp_h": round(r["exp_h"]), "die_over_kill": round(r["die_over_kill"], 2), "pvp": round(r["pvp"], 3),
                                     "hp": r["hp"], "pve_rel": round(r["pve_rel"], 3), "pvp_rel": round(r["pvp_rel"], 3),
                                     "score": round(r["score"], 3)})
                best = max(rows, key=lambda r: r["score"])
                chosen[(p.cls, level, rnd)] = (best["shares"], best["lapis"])
            opponents = [p.spec(*chosen[(p.cls, level, rnd)]) for p in ps]
            print(f"search {level} round {rnd} done", file=sys.stderr)
    write_csv("builds_search.csv", rows_out)
    out = []
    for cls in CLASSES:
        for level in LEVELS:
            shares, lapis = chosen[(cls, level, 2)]
            first = chosen[(cls, level, 1)]
            out.append({"class": cls, "mode": "ultimate", "level": level, "alloc": shares_str(shares), "lapis": lapis,
                        "round1": f"{shares_str(first[0])} {first[1]}"})
        # Level 55 (PvE only) takes the level-60 choice; the other modes the ultimate choice of their level.
        shares, lapis = chosen[(cls, 60, 2)]
        out.append({"class": cls, "mode": "ultimate", "level": 55, "alloc": shares_str(shares), "lapis": lapis, "round1": ""})
        for mode in ("normal", "hard"):
            for level in LEVELS:
                shares, lapis = chosen[(cls, level, 2)]
                out.append({"class": cls, "mode": mode, "level": level, "alloc": shares_str(shares), "lapis": lapis, "round1": ""})
    for r in out:
        p = plans[(r["class"], r["mode"], int(r["level"]))]
        shares = {k: int(v) for k, v in (t.split(":") for t in r["alloc"].split(","))}
        r["points_left"] = p.left
        r["pvp_spec"] = p.spec(shares, r["lapis"])
        r["pve_spec"] = p.spec(shares, r["lapis"], pvp=False, element=(best_element(data.mobs()[FIELD[int(r["level"])]]), ELEMENT))
    write_csv("builds.csv", out)


def _final_one(args) -> list:
    cls, level, pve_spec = args
    rows = []
    for gear in ("expected", "good", "plain"):
        for party in (False, True):
            if party and (gear != "expected" or level < 55):
                continue
            s = gear_variant(pve_spec, gear)
            if party:
                b = model.parse_build(s)
                s = add_buffs(s, buffs_for(b.job, b.family, b.grow, level, b.learnt, False, True))
            for mob_id in MOBS[level]:
                mob = data.mobs()[mob_id]
                if gear != "plain":
                    s = set_opt(s, "element", f"{best_element(mob)},{ELEMENT}")
                c = model.evaluate(model.parse_build(s))
                f = mob_fight(c, mob)
                need = None
                if level < 60:
                    t = data.exp_table()
                    need = t.total(c.build.grow, level) - t.total(c.build.grow, level - 1)
                top = sorted(f["rotation"].items(), key=lambda kv: -kv[1])[:3]
                rows.append({"class": cls, "level": level, "gear": gear, "party": int(party), "mob": mob_id, "mob_level": mob.level,
                             **sheet_row(c), "hit_rate": f["hit"], "kill_s": round(f["kill_s"], 1),
                             "kill_unlimited_s": round(f["kill_unlimited_s"], 1), "die_s": round(f["die_s"], 1),
                             "mob_dps": round(f["mob_dps"]), "exp_kill": f["exp"],
                             "rest_s": round(f["rest_s"], 1) if f["rest_s"] is not None else "",
                             "exp_h": round(f["exp_h"]), "hours_per_level": round(need / f["exp_h"], 1) if need and f["exp_h"] else "",
                             "rotation": "; ".join(f"{n.strip()} ×{u}" for n, u in top), "spec": s})
    return rows


def stage_final(pool) -> None:
    todo = [(r["class"], int(r["level"]), r["pve_spec"]) for r in read_csv("builds.csv") if r["mode"] == "ultimate"]
    write_csv("builds_pve.csv", [x for part in pool.map(_final_one, todo, chunksize=1) for x in part])


def _pvp_row(args) -> list:
    attacker, targets, variant, level = args
    a = model.evaluate(model.parse_build(attacker))
    learnt = dict(a.build.learnt)
    debuffs = [(i, learnt[i]) for i in DEBUFFS[a.build.job] if i in learnt] if variant == "debuffs" else []
    out = []
    for name, t in targets:
        b = model.parse_build(t)
        # The attacker's opening debuffs, landed, on the target (`buff_bonuses` takes their abilities off).
        b.buffs = list(b.buffs) + debuffs
        out.append({"level": level, "variant": variant, "attacker": model.class_name(a.build.job, a.build.family), "target": name,
                    "kill_s": round(ttk(a, model.evaluate(b)), 2)})
    return out


def variant_drop(job: int, variant: str) -> tuple:
    drop = set(PERCENT_SKILLS) if variant in ("no_percent", "no_percent_dot") else set()
    if variant == "no_percent_dot" and job == 4:
        drop |= set(MAGE_DOTS)
    return tuple(sorted(drop))


def _variant_one(args) -> dict:
    """The build without some skills, its points spent again (`choose_skills` with `exclude`)."""
    plan, shares, lapis, variant = args
    learnt, left, _ = choose_skills(plan.cls, plan.mode, plan.level, plan.real, plan.shield, exclude=variant_drop(plan.job, variant))
    p = Plan(plan.cls, plan.mode, plan.level, plan.real, plan.shield, learnt, left, {}, [])
    return {"class": plan.cls, "level": plan.level, "variant": variant, "points_left": left, "pvp_spec": p.spec(shares, lapis)}


def stage_variants(pool) -> None:
    plans = load_plans()
    todo = []
    for r in read_csv("builds.csv"):
        if r["mode"] != "ultimate" or int(r["level"]) not in LEVELS:
            continue
        shares = {k: int(v) for k, v in (t.split(":") for t in r["alloc"].split(","))}
        for v in ("no_percent", "no_percent_dot"):
            todo.append((plans[(r["class"], "ultimate", int(r["level"]))], shares, r["lapis"], v))
    write_csv("builds_variants.csv", pool.map(_variant_one, todo, chunksize=1))


def stage_pvp(pool) -> None:
    specs = {(r["class"], int(r["level"]), "all"): r["pvp_spec"] for r in read_csv("builds.csv") if r["mode"] == "ultimate"}
    specs.update({(r["class"], int(r["level"]), r["variant"]): r["pvp_spec"] for r in read_csv("builds_variants.csv")})
    todo = []
    for level in LEVELS:
        for v in ["all", "no_percent_dot"] + (["no_percent", "debuffs", "good", "plain"] if level == 60 else []):
            if v in ("no_percent", "no_percent_dot"):
                these = {c: specs[(c, level, v)] for c in CLASSES}
            else:
                these = {c: gear_variant(specs[(c, level, "all")], v) for c in CLASSES}
            targets = list(these.items())
            todo += [(these[c], targets, v, level) for c in CLASSES]
    write_csv("builds_pvp.csv", [x for part in pool.map(_pvp_row, todo, chunksize=1) for x in part])


RACE_SKILLS = (128, 129, 188, 189, 190, 191)


def _faction_one(args) -> list:
    """A pair's kill times each way, from equal to real: the Fury class given the Light build whole (stats split, skills, buffs,
    items) and neither casting race skills, so what is left is the race's starting stats and HP; then each side's race skills; then
    each its own build; then both at kill-count grade 13 and 30."""
    light, fury, level, ls, fs = args
    rows = []
    fury_race = model.parse_build(fs)

    def race_free(b):
        b.learnt = [x for x in b.learnt if x[0] not in RACE_SKILLS]
        b.buffs = [x for x in b.buffs if x[0] not in RACE_SKILLS]
        return b

    for step, same, race, grade in (("same build, no race skills", True, False, 0), ("+ race skills", True, True, 0),
                                    ("+ own builds (the real ones)", False, True, 0), ("+ grade 13 both", False, True, 13),
                                    ("+ grade 30 both", False, True, 30)):
        lb = model.parse_build(ls)
        if same:
            fb = race_free(model.parse_build(fury + " " + ls.split(" ", 1)[1]))
            fb.items = [replace(it) for it in lb.items]
            if race:
                fb.learnt += [x for x in fury_race.learnt if x[0] in RACE_SKILLS]
                fb.buffs += [x for x in fury_race.buffs if x[0] in RACE_SKILLS]
            else:
                race_free(lb)
        else:
            fb = model.parse_build(fs)
        lb.grade = fb.grade = grade
        lc, fc = model.evaluate(lb), model.evaluate(fb)
        rows.append({"pair": f"{light} / {fury}", "level": level, "step": step, "light_hp": lc.max_hp, "fury_hp": fc.max_hp,
                     "light_attack": sheet_row(lc)["attack"], "fury_attack": sheet_row(fc)["attack"],
                     "light_defence": lc.fighter.kinds[fm.MELEE].defence, "fury_defence": fc.fighter.kinds[fm.MELEE].defence,
                     "light_kills_s": round(ttk(lc, fc), 2), "fury_kills_s": round(ttk(fc, lc), 2)})
    return rows


def stage_factions(pool) -> None:
    specs = {(r["class"], int(r["level"])): r["pvp_spec"] for r in read_csv("builds.csv") if r["mode"] == "ultimate"}
    todo = [(l, f, lv, specs[(l, lv)], specs[(f, lv)]) for lv in LEVELS for l, f in PAIRS]
    write_csv("builds_factions.csv", [x for part in pool.map(_faction_one, todo, chunksize=1) for x in part])


def _mode_one(args) -> dict:
    cls, mode, level, pvp_s, pve_s, ult = args
    c = model.evaluate(model.parse_build(pvp_s))
    f = mob_fight(model.evaluate(model.parse_build(pve_s)), data.mobs()[FIELD[level]])
    ults = [model.evaluate(model.parse_build(s)) for s in ult]
    dealt = [ttk(c, u) for u in ults]
    taken = [ttk(u, c) for u in ults]
    mirror = ults[(CLASSES.index(cls) + 6) % 12]
    t = data.exp_table()
    need = t.total(c.build.grow, level) - t.total(c.build.grow, level - 1) if level < 60 else None
    return {"class": cls, "mode": mode, "level": level, **sheet_row(c), "kill_s": round(f["kill_s"], 1), "die_s": round(f["die_s"], 1),
            "exp_h": round(f["exp_h"]), "hours_per_level": round(need / f["exp_h"], 1) if need and f["exp_h"] else "",
            "kills_ultimate_median_s": round(statistics.median(dealt), 1), "killed_by_ultimate_median_s": round(statistics.median(taken), 1),
            "mirror_kills_s": round(ttk(c, mirror), 1), "killed_by_mirror_s": round(ttk(mirror, c), 1), "spec": pvp_s}


def stage_modes(pool) -> None:
    rows = read_csv("builds.csv")
    ult = {(r["class"], int(r["level"])): r["pvp_spec"] for r in rows if r["mode"] == "ultimate"}
    todo = [(r["class"], r["mode"], int(r["level"]), r["pvp_spec"], r["pve_spec"], [ult[(c, int(r["level"]))] for c in CLASSES])
            for r in rows if int(r["level"]) in LEVELS]
    write_csv("builds_modes.csv", pool.map(_mode_one, todo, chunksize=1))


def scaled_mob(mob: fm.MobDef, defence: float, attack: float) -> fm.MobDef:
    return replace(mob, defense=int(mob.defense * defence), magic=int(mob.magic * defence),
                   attacks=tuple(replace(e, damage=int(e.damage * attack), damage_add=int(e.damage_add * attack)) for e in mob.attacks))


WALL_STEPS = ((1, 1), (0.5, 1), (1, 0.5), (0.5, 0.5), (0.25, 0.5), (0.25, 0.35), (0.15, 0.35))


def _lever_wall(args) -> list:
    """With plain gear (no enchant, crafted numbers or lapis: a character arriving at the fields), where the wall is."""
    cls, s = args
    c = model.evaluate(model.parse_build(gear_variant(s, "plain")))
    out = []
    for d, a in WALL_STEPS:
        f = mob_fight(c, scaled_mob(data.mobs()[1501], d, a), full=False)
        out.append({"class": cls, "defence_x": d, "attack_x": a, "kill_s": round(f["kill_s"], 1), "die_s": round(f["die_s"], 1),
                    "exp_h": round(f["exp_h"])})
    return out


def _lever_skill(args) -> dict:
    """A class's median and fastest kill time against the other eleven when skills' damage (and damage over time) is multiplied."""
    name, cls, level, specs, patch = args
    sk = data.skills()
    saved = {k: sk[k] for k, _ in patch}
    for k, factor in patch:
        s = sk[k]
        sk[k] = replace(s, damage=tuple(int(x * factor) for x in s.damage), time_damage=tuple(int(x * factor) for x in s.time_damage))
    a = model.evaluate(model.parse_build(specs[cls]))
    times = [ttk(a, model.evaluate(model.parse_build(specs[o]))) for o in CLASSES if o != cls]
    sk.update(saved)
    return {"lever": name, "class": cls, "level": level, "median_kill_s": round(statistics.median(times), 2), "fastest_s": round(min(times), 2)}


def stage_levers(pool) -> None:
    rows = read_csv("builds.csv")
    pve55 = [(r["class"], r["pve_spec"]) for r in rows if r["mode"] == "ultimate" and r["level"] == "55"]
    write_csv("builds_lever_wall.csv", [x for part in pool.map(_lever_wall, pve55, chunksize=1) for x in part])
    todo = []
    dots = [((sid, lv), 0.5) for sid in (321, 146, 82, 68) for lv in (1, 2, 3) if (sid, lv) in data.skills()]
    for level in (30, 60):
        specs = {r["class"]: r["pvp_spec"] for r in rows if r["mode"] == "ultimate" and int(r["level"]) == level}
        for cls in ("mage", "pagan"):
            todo += [("as is", cls, level, specs, []), ("damage over time × 0.5", cls, level, specs, dots)]
        if level == 60:
            for cls, sid in (("fighter", 180), ("warrior", 180), ("ranger", 182), ("assassin", 182), ("archer", 183), ("hunter", 183),
                             ("mage", 184), ("pagan", 184)):
                todo += [("as is", cls, level, specs, []), ("percentage skill × 0.5", cls, level, specs, [((sid, 2), 0.5), ((sid, 1), 0.5)])]
    write_csv("builds_lever_skills.csv", pool.map(_lever_skill, todo, chunksize=1))
    # The HP each build has from its gear: crafted HP, MP and SP at 50 a point instead of 100 (`formulas.CRAFT_HEALTH`), and plain gear.
    hp = []
    for r in rows:
        if r["mode"] != "ultimate" or int(r["level"]) not in LEVELS:
            continue
        c = model.evaluate(model.parse_build(r["pvp_spec"]))
        old, fm.CRAFT_HEALTH = fm.CRAFT_HEALTH, 50
        c50 = model.evaluate(model.parse_build(r["pvp_spec"]))
        fm.CRAFT_HEALTH = old
        plain = model.evaluate(model.parse_build(gear_variant(r["pvp_spec"], "plain")))
        hp.append({"class": r["class"], "level": r["level"], "hp": c.max_hp, "hp_craft_50": c50.max_hp, "hp_plain_gear": plain.max_hp})
    write_csv("builds_lever_hp.csv", hp)


STAGES = {"plans": stage_plans, "search": stage_search, "final": stage_final, "variants": stage_variants, "pvp": stage_pvp, "factions": stage_factions,
          "modes": stage_modes, "levers": stage_levers}


def _warm():
    data.items(), data.skills(), data.mobs(), data.sources(), model._linkable_lapis()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--only", default="", help="stages, comma-separated: " + ",".join(STAGES))
    ap.add_argument("--report", action="store_true", help="print the document's tables from the CSVs")
    args = ap.parse_args()
    if args.report:
        import builds_report
        builds_report.main()
        return
    with Pool(args.jobs, initializer=_warm) as pool:
        for name in (args.only.split(",") if args.only else STAGES):
            STAGES[name](pool)
            print(f"stage {name} done", file=sys.stderr)


if __name__ == "__main__":
    main()
