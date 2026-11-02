"""Expected values of the rolls in formulas.py: hit and critical chances exactly, damage means by averaging the formula over the
random terms (the attack spread `rand() % var`, the luck roll `rand() % LUC + 1`, the critical's second luck roll).

How: the spread and the luck roll are averaged over a grid of at most GRID values each (every value when there are fewer, otherwise the
midpoints of equal strata); the critical's second roll enters linearly, so its mean is used. The floor of 0, the truncation, the 30,000
cap, "10 or less becomes 1 to 10" (mean 5.5) and absorption are applied at each grid point, so the clamps are honoured. The modulo
bias of rand() % n (32768 is not a multiple of n) is ignored. test_formulas.py checks these means against sampling the exact functions.

Every function returns an `Expect`: chances in 0..1 and damage means; `per_attempt` = hit × the mean of a hit.
"""
from __future__ import annotations

from dataclasses import dataclass

import formulas as fm
from formulas import F_0_1, F_0_7, F_1_4, f32, i32

GRID = 17


@dataclass
class Expect:
    hit: float  # chance the attack lands (after blocks the caller removes)
    crit: float  # chance a landed attack is a critical
    normal: float  # mean damage of a landed non-critical
    critical: float  # mean damage of a landed critical
    sp: float = 0.0  # SP and MP damage of a landed skill
    mp: float = 0.0

    @property
    def landed(self) -> float:
        return self.normal * (1 - self.crit) + self.critical * self.crit

    @property
    def per_attempt(self) -> float:
        return self.hit * self.landed


def uniform(lo: int, hi: int):
    """Values and weights of a uniform whole number from lo to hi (inclusive)."""
    n = hi - lo + 1
    if n <= 0:
        return [(lo, 1.0)]
    if n <= GRID:
        return [(lo + k, 1.0 / n) for k in range(n)]
    step = n / GRID
    return [(lo + (k + 0.5) * step - 0.5, 1.0 / GRID) for k in range(GRID)]


def spread(var: int):
    return uniform(0, var - 1) if var > 0 else [(0, 1.0)]


def luck(luc: int):
    return uniform(1, luc) if luc > 0 else [(0, 1.0)]


def luck_mean(luc: int) -> float:
    return (luc + 1) / 2.0 if luc > 0 else 0.0


def finish(value: float, absorption: int = 0) -> float:
    """Truncation, at most 30,000, 10 or less → 1 to 10 (mean 5.5), then absorption (at least 1 left)."""
    d = min(i32(value), fm.MAX_DAMAGE)
    if d <= 10:
        return sum(1.0 if x <= absorption else float(x - max(absorption, 0)) for x in range(1, 11)) / 10.0
    if absorption > 0:
        return 1.0 if d <= absorption else float(d - absorption)
    return float(d)


def crit_chance(attacker: fm.Fighter, defender_luc: int, lo: int) -> float:
    c = fm.clamp(i32(float(attacker.critical) / 100.0 * 100.0 - float(defender_luc) * fm.F_0_034), lo, 99)
    return c / 100.0


# --- a player hitting a monster (combat::formula) ------------------------------------------------------------------------------------


def pve_damage(p: fm.Fighter, m: fm.Fighter, kind: int, close: bool, bonus: int, elem: float) -> tuple[float, float, float]:
    """(critical chance, mean non-critical, mean critical) of `combat::formula` after its caller's minimum (the ≤ 10 rule)."""
    level = fm.level_factor(p.level, m.level)
    short = kind == fm.RANGED and close
    defence = float(m.kinds[kind].defence)
    r2 = luck_mean(p.luc) * F_1_4
    normal = critical = 0.0
    for u, wu in spread(p.attack_var):
        attack = f32(f32(float(p.kinds[kind].attack + bonus)) + float(u))
        if short:
            attack = f32(attack * F_0_7)
        for r, wr in luck(p.luc):
            d = max(f32((attack - attack / 100.0 * level) * elem - (defence + defence / 100.0 * level) + r * F_0_1), 0.0)
            c = d * 1.5 + r2 - r2 / 100.0 * level
            if short:
                c *= F_0_7
            normal += wu * wr * finish(d)
            critical += wu * wr * finish(c)
    return crit_chance(p, m.luc, 5), normal, critical


def pve_attack(p: fm.Fighter, m: fm.Fighter, kind: int, close: bool = False) -> Expect:
    """A normal attack on a monster (`combat::strike`)."""
    crit, normal, critical = pve_damage(p, m, kind, close, 0, fm.element_factor(p.element, m.element))
    return Expect(fm.hit_rate(p, m, kind) / 100.0, crit, normal, critical)


def skill_hit_chance(p: fm.Fighter, m: fm.Fighter, d: fm.MobDef, skill: fm.SkillDef) -> float:
    """`combat::skill_hits` as a chance."""
    st = skill.state_type
    if (st != 0 and st - 1 < len(d.resist_states) and d.resist_states[st - 1]) or skill.id in d.resist_skills:
        return 0.0
    if skill.success_type == 1:
        return min(skill.success_value, 100) / 100.0
    kind = min(skill.attack_kind, fm.MAGIC)
    if skill.success_type in (2, 3):
        ours, theirs = (p.luc, m.luc) if skill.success_type == 2 else (p.wis, m.wis)
        extra = 100 * (ours // (theirs + ours)) if ours + theirs else 0
        return min(skill.success_value + extra, 100) / 100.0
    if kind == fm.MAGIC:
        return 1 - fm.magic_resist_rate(p, m) / 100.0
    return fm.hit_rate(p, m, kind) / 100.0


def pve_skill(p: fm.Fighter, m: fm.Fighter, d: fm.MobDef, skill: fm.SkillDef, caster=(0, 0, 0), mob_health=None,
              close: bool = False) -> Expect:
    """One hit of a skill on a monster (`combat::skill_hits`, `combat::skill_damage`). `caster` and `mob_health`: current HP, SP, MP."""
    hit = skill_hit_chance(p, m, d, skill)
    w = skill.damage
    mob_health = mob_health or (d.hp, d.sp, d.mp)

    def each(base, f):
        return [fm._capped(max(f(w[i], base[i]), 1)) if w[i] != 0 else 0 for i in range(3)]

    if skill.damage_type in (0, 3, 4, 5):
        if skill.damage_type == 0:
            v = list(w)
        elif skill.damage_type == 3:
            v = each(caster, lambda a, b: a * b)
        elif skill.damage_type == 4:
            v = each((d.hp, d.sp, d.mp), lambda a, b: int(a / 100.0 * b))
        else:
            v = each(mob_health, lambda a, b: int(a / 100.0 * b))
        return Expect(hit, 0.0, float(v[0]), float(v[0]), float(v[1]), float(v[2]))
    kind = min(skill.attack_kind, fm.MAGIC)
    attack_element = skill.element if skill.element != 0 else p.element
    factor = fm.element_factor(attack_element, 0 if skill.kind == fm.NEUTRAL_ELEMENT else m.element)
    if skill.damage_type == 2:
        crit, normal, critical = pve_damage(p, m, kind, close, 0, factor)
        mult = w[0] if w[0] != 0 else 1
        return Expect(hit, crit, min(normal * mult, fm.MAX_DAMAGE), min(critical * mult, fm.MAX_DAMAGE))
    crit, normal, critical = pve_damage(p, m, kind, close, w[0], factor)
    return Expect(hit, crit, normal, critical, float(w[1]), float(w[2]))


# --- a monster hitting a player (combat::mob_strike, mob_skill_damage) --------------------------------------------------------------


def mob_attack(m: fm.Fighter, damage: int, damage_add: int, p: fm.Fighter, defence_element: int, absorption: int = 0,
               strength: float = 1.0) -> Expect:
    """A monster's normal attack on a player, after absorption (`combat::mob_strike`, `combat::absorb`)."""
    level = fm.level_factor(p.level, m.level)
    elem = fm.ELEMENTS[0][defence_element] if defence_element < 9 else 1.0
    defence = float(p.kinds[fm.MELEE].defence)
    r2 = luck_mean(m.luc) * F_1_4
    normal = critical = 0.0
    for u, wu in spread(damage_add):
        attack = f32(f32(f32(float(damage) * strength) + float(m.kinds[fm.MELEE].attack)) + float(u))
        for r, wr in luck(m.luc):
            h = max(f32((attack + attack / 100.0 * level) * elem - (defence - defence / 100.0 * level) + r * F_0_1), 0.0)
            normal += wu * wr * finish(h, absorption)
            critical += wu * wr * finish(r2 / 100.0 * level + h * 1.5 + r2, absorption)
    return Expect(fm.mob_hit_rate(m, p, fm.MELEE) / 100.0, crit_chance(m, p.luc, 5), normal, critical)


# --- players hitting players (combat::pvp_strike, pvp_skill_damage) ----------------------------------------------------------------


def _pvp_physical(a: fm.Fighter, t: fm.Fighter, kind: int, bonus: int, factor: float, close: bool, cross_road: bool,
                  absorption: int) -> tuple[float, float, float]:
    r2 = luck_mean(a.luc) * F_1_4
    normal = critical = 0.0
    for u, wu in spread(a.attack_var):
        d0 = max(bonus + a.kinds[kind].attack - t.kinds[kind].defence + int(u), 0)
        if kind == fm.RANGED and close and not cross_road:
            d0 = i32(d0 * F_0_7)
        d0 = i32(d0 * factor)
        for r, wr in luck(a.luc):
            d = i32(r * F_0_1 + d0)
            c = d * 0.5 + r2 + d
            if kind == fm.RANGED and close:
                c *= F_0_7
            normal += wu * wr * finish(d, absorption)
            critical += wu * wr * finish(c, absorption)
    return crit_chance(a, t.luc, 1), normal, critical


def pvp_attack(a: fm.Fighter, t: fm.Fighter, kind: int, defence_element: int = 0, close: bool = False, cross_road: bool = False,
               absorption: int = 0, block: float = 0.0) -> Expect:
    """A normal blow on a player after the target's block chance (`block`, 0..1), its absorption and its element."""
    crit, normal, critical = _pvp_physical(a, t, kind, 0, fm.element_factor(a.element, defence_element), close, cross_road, absorption)
    return Expect((1 - block) * fm.pvp_hit_rate(a, t, kind) / 100.0, crit, normal, critical)


def pvp_skill_hit_chance(c: fm.Fighter, t: fm.Fighter, skill: fm.SkillDef) -> float:
    if skill.success_type == 1:
        return min(skill.success_value, 100) / 100.0
    if skill.success_type in (2, 3):
        ours, theirs = (c.luc, t.luc) if skill.success_type == 2 else (c.wis, t.wis)
        extra = 100 * (ours // (theirs + ours)) if ours + theirs else 0
        return min(skill.success_value + extra, 100) / 100.0
    if skill.attack_kind == fm.MAGIC:
        return 1 - fm.pvp_resist_rate(c, t) / 100.0
    if skill.attack_kind in (fm.MELEE, fm.RANGED):
        return fm.pvp_hit_rate(c, t, skill.attack_kind) / 100.0
    return 1.0


def pvp_skill(h: fm.PvpSkillHit, skill: fm.SkillDef, absorption: int = 0, block: float = 0.0) -> Expect:
    """One hit of a skill on a player (`combat::pvp_skill_hits`, `combat::pvp_skill_damage`), after absorption."""
    a, t = h.caster, h.target
    hit = (1 - block) * pvp_skill_hit_chance(a, t, skill)
    w = skill.damage

    def each(base, f):
        return [fm._capped(max(f(w[i], base[i]), 1)) if w[i] != 0 else 0 for i in range(3)]

    def pct(wv, of):
        return int(f32(f32(wv / 100.0) * of))

    if skill.damage_type in (0, 3, 4, 5):
        v = {0: lambda: list(w), 3: lambda: each(h.caster_health, lambda x, y: x * y), 4: lambda: each(h.target_max, pct),
             5: lambda: each(h.target_health, pct)}[skill.damage_type]()
        hp = finish(v[0], absorption) if v[0] > 0 else 0.0
        return Expect(hit, 0.0, hp, hp, float(v[1]), float(v[2]))
    attack_element = skill.element if skill.element != 0 else a.element
    factor = fm.element_factor(attack_element, 0 if skill.kind == fm.NEUTRAL_ELEMENT else h.defence_element)
    bonus = w[0] if skill.damage_type == 1 else 0
    mult = w[0] if skill.damage_type == 2 and w[0] != 0 else 1
    if skill.attack_kind in (fm.MELEE, fm.RANGED):
        crit, normal, critical = _pvp_physical(a, t, skill.attack_kind, bonus, factor, h.within_three, h.cross_road, 0)
    elif skill.attack_kind == fm.MAGIC:
        p1, p2 = fm.pvp_magic_cut(a, t)
        x = float(f32(f32(factor) * f32(float(a.kinds[fm.MAGIC].attack + bonus))))
        d0 = max(float(f32(x - x / 100.0 * (p1 + p2))), 0.0)
        r2 = luck_mean(a.luc) * F_1_4
        normal = critical = 0.0
        for r, wr in luck(a.luc):
            d = float(f32(r * F_0_1 + d0))
            normal += wr * finish(d)
            critical += wr * finish(d * 0.5 + r2 + d)
        crit = crit_chance(a, t.luc, 1)
    else:
        crit, normal, critical = 0.0, float(bonus), float(bonus)
    if mult != 1 or absorption:
        # Multiplied first, then absorbed (the multiplication is part of the damage; absorption comes in CUser::Damage).
        normal = finish(normal * mult, absorption)
        critical = finish(critical * mult, absorption)
    sp, mp = (float(w[1]), float(w[2])) if skill.damage_type == 1 else (0.0, 0.0)
    return Expect(hit, crit, normal, critical, sp, mp)
