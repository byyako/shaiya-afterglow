"""The game's numbers as the server computes them: a line-by-line port of crates/game-server/src/stats.rs, combat.rs,
skills.rs (passives, abilities), buffs.rs (what applied skills add up to), lapis.rs (enchant steps, elements), kc.rs (percent),
creation.rs (starting stats) and item_use.rs (stat resets). Each function names its Rust original; the Rust code is the truth and
test_formulas.py replays its unit tests.

The Rust code computes in double precision and rounds to single precision where the original stores (`as f32`); `f32()` below does
the same, and `i32()` is Rust's `as i32` (truncation toward zero, NaN to 0, saturating). `rand` is a callable returning the C
library's rand() (0 to 32767); the rolls are taken in the same order as the Rust code, so a test can feed the same sequence.

Arrays follow the server: stats STR, DEX, REC, INT, WIS, LUC; health (maxima) HP, MP, SP; damage values HP, SP, MP.
"""
from __future__ import annotations

import math
import struct
from dataclasses import dataclass, field, replace

# --- Rust arithmetic -------------------------------------------------------------------------------------------------------------


def f32(x: float) -> float:
    """`x as f32` (round to nearest single precision), back as a Python float."""
    if math.isnan(x) or math.isinf(x):
        return x
    try:
        return struct.unpack("<f", struct.pack("<f", x))[0]
    except OverflowError:
        return math.copysign(math.inf, x)


def i32(x: float) -> int:
    """`x as i32` from a float: truncation toward zero, NaN 0, saturating."""
    if math.isnan(x):
        return 0
    if x >= 2147483647:
        return 2147483647
    if x <= -2147483648:
        return -2147483648
    return int(x)


def u16(x: float) -> int:
    if math.isnan(x) or x <= 0:
        return 0
    return min(int(x), 65535)


def u32(x: float) -> int:
    if math.isnan(x) or x <= 0:
        return 0
    return min(int(x), 4294967295)


def fmax(a: float, b: float) -> float:
    """Rust's f32/f64 `max`: a NaN operand gives the other."""
    if math.isnan(a):
        return b
    if math.isnan(b):
        return a
    return a if a >= b else b


def fclamp(x: float, lo: float, hi: float) -> float:
    """Rust's float `clamp`: NaN stays NaN."""
    if math.isnan(x):
        return x
    return lo if x < lo else hi if x > hi else x


def clamp(x: int, lo: int, hi: int) -> int:
    return lo if x < lo else hi if x > hi else x


F_0_01 = f32(0.01)
F_0_034 = f32(0.034)
F_0_05 = f32(0.05)
F_0_1 = f32(0.1)
F_0_19 = f32(0.19)
F_0_2 = f32(0.2)
F_0_3 = f32(0.3)
F_0_7 = f32(0.7)
F_1_1 = f32(1.1)
F_1_3 = f32(1.3)
F_1_4 = f32(1.4)
F_1_5 = f32(1.5)
F_2_4 = f32(2.4)

STR, DEX, REC, INT, WIS, LUC = range(6)
HP, MP, SP = range(3)  # maxima order
MELEE, RANGED, MAGIC = range(3)

RESULT_HIT, RESULT_CRITICAL, RESULT_MISS = 0, 1, 3
MAX_DAMAGE = 30_000

# --- items (inventory.rs) --------------------------------------------------------------------------------------------------------

_REAL_TYPES = [
    [1, 45], [2, 46], [3, 47], [4, 48], [5, 49, 50], [6, 51, 52], [7, 53, 54], [8, 55, 56], [9, 57], [10, 58], [11, 59],
    [12, 60, 61], [13, 62, 63], [14, 64], [15, 65], [16, 31, 66, 72, 81, 87], [17, 32, 67, 73, 82, 88], [18, 33, 68, 74, 83, 89],
    [19, 34, 69, 75, 84, 90], [20, 35, 70, 76, 85, 91], [21, 36, 71, 77, 86, 92], [22, 37], [23, 38], [24, 39],
]


def _real_type_table() -> dict[int, int]:
    """`LITEM_TYPE` (ps_game.exe 0x542188) as inventory.rs has it; types 40 and 41 are bracelets (25), 25 to 29 the usable and quest
    items (26, 27; only what the model needs)."""
    table = {}
    for real, types in enumerate(_REAL_TYPES, start=1):
        for t in types:
            table[t] = real
    for t in (40, 41):
        table[t] = 25
    return table


REAL_TYPE = _real_type_table()


def real_type(item_type: int) -> int:
    """`inventory::real_type` for the equipment types (1 to 15 weapons, 16 to 25 armour and accessories); 0 otherwise."""
    return REAL_TYPE.get(item_type, 0)


@dataclass
class ItemDef:
    """`inventory::ItemDef`, the fields the numbers read. Health is HP, MP, SP."""
    item_type: int
    type_id: int
    level: int = 0
    country: int = 6
    jobs: tuple = (True,) * 6
    grow: int = 0
    special: int = 0
    const_stats: tuple = (0,) * 6
    const_health: tuple = (0, 0, 0)
    attack_time: int = 0
    speed: int = 0
    craftable: bool = False
    effects: tuple = (0, 0, 0, 0)
    range: int = 0
    attrib: int = 0
    quality: int = 0
    absorption: int = 0
    item_id: int = 0
    name: str = ""
    buy: int = 0
    grade: int = 0
    craft_max: int = 0
    room: int = 0
    server: int = 0  # how many crafted numbers a drop gets (`lapis::roll_craft`)
    req_str: int = 0  # a lapis: fits a bracelet (`lapis::fits`)
    reqvg: int = 0  # a lapis or Lapisia: dangerous
    reqig: int = 0  # a lapis: its link grade


@dataclass
class ItemUnit:
    """`backend::ItemUnit`: an item a character holds."""
    item_type: int
    type_id: int
    slot: int
    bag: int = 0
    quality: int = 0
    gems: tuple = (0,) * 6
    craft_name: bytes = b""


class ItemDefs:
    def __init__(self, defs=()):
        self.by_key = {(d.item_type, d.type_id): d for d in defs}

    def get(self, item_type: int, type_id: int):
        return self.by_key.get((item_type, type_id))


# --- skills (skills.rs) ----------------------------------------------------------------------------------------------------------

PASSIVE, ATTACK, HEAL, DEBUFF, BUFF, FIELD, ONCE = range(7)
MULTI_HIT, DESPERATE, NEUTRAL_ELEMENT, DRAIN = 22, 23, 24, 27
ABILITIES, ABILITIES_TOO, LOW_HP = 82, 91, 6
MOB_ABILITIES, MOB_ABILITIES_TOO, STUN, STOP, SLEEP, NO_ELEMENT = 62, 67, 64, 65, 66, 68
WEAPON_TYPES = 20
MOB_LEVEL = 100


@dataclass
class SkillDef:
    """`skills::SkillDef` after `corrected()` (attack_kind and defense_type 0 to 2)."""
    id: int = 0
    level: int = 1
    country: int = 6
    jobs: tuple = (True,) * 6
    grow: int = 0
    points: int = 0
    required_level: int = 0
    type_show: int = 1
    type_effect: int = 1
    kind: int = 0
    weapons: tuple = (0, 0)
    weapon_value: int = 0
    abilities: tuple = ((0, 0), (0, 0), (0, 0))
    prev_skill: int = 0
    attack_kind: int = 0
    need_weapons: tuple = (False,) * 15
    need_shield: bool = False
    need_sp: int = 0
    need_mp: int = 0
    ready_time: int = 0
    reset_time: int = 0
    attack_range: int = 0
    apply_range: int = 0
    state_type: int = 0
    element: int = 0
    success_type: int = 0
    success_value: int = 0
    target_type: int = 0
    multi_attack: int = 0
    damage_type: int = 0
    damage: tuple = (0, 0, 0)
    add_damage: tuple = (0, 0, 0)
    heal: tuple = (0, 0, 0)
    keep_time: int = 0
    time_damage_type: int = 0
    time_damage: tuple = (0, 0, 0)
    time_heal: tuple = (0, 0, 0)
    defense_type: int = 0
    defense_value: int = 0
    limit_hp: int = 0
    keep_on_death: bool = False
    name: str = ""


def corrected(d: SkillDef) -> SkillDef:
    """`SkillDef::corrected` (`CGameData::LoadConstSkill`); `d` holds the raw columns (attack kind 1 to 3)."""
    abilities = tuple((t, 1 if t != 0 and v == 0 else v) for t, v in d.abilities)
    type_effect = 4 if d.kind == 93 else 6 if d.kind == 122 else d.type_effect
    return replace(
        d,
        weapons=tuple(min(w, WEAPON_TYPES - 1) for w in d.weapons),
        abilities=abilities,
        type_effect=type_effect,
        attack_kind=clamp(d.attack_kind, 1, 3) - 1,
        defense_type=clamp(d.defense_type, 1, 3) - 1,
        element=0 if d.element >= 9 else d.element,
        keep_time=d.keep_time * 3600 if d.keep_on_death else d.keep_time,
    )


@dataclass
class Numbers:
    """`combat::Numbers` (`stAttack`)."""
    hit: int = 0
    evasion: int = 0
    attack: int = 0
    defence: int = 0


def _numbers3():
    return [Numbers(), Numbers(), Numbers()]


@dataclass
class Bonuses:
    """`skills::Bonuses` (`CUser::SetSkillAbility`). Health is HP, MP, SP."""
    max_health: list = field(default_factory=lambda: [0, 0, 0])
    stats: list = field(default_factory=lambda: [0] * 6)
    sit_recovery_percent: list = field(default_factory=lambda: [0, 0, 0])
    cost_reduction: list = field(default_factory=lambda: [0, 0])
    attack_range: int = 0
    attack_speed: int = 0
    move_speed: int = 0
    critical: int = 0
    cool_time: int = 0
    adds: list = field(default_factory=_numbers3)
    disabled: list = field(default_factory=lambda: [0, 0, 0])

    def add(self, ability: int, value: int, type_effect: int) -> None:
        """`Bonuses::add`: `type_effect` 3 leaves the maxima alone."""
        maxima = type_effect != 3

        def stat(s, health):
            self.stats[s] += value
            if health is not None and maxima:
                self.max_health[health] += value * 5

        if ability == 1 and maxima:
            self.max_health[HP] += value
        elif ability == 2 and maxima:
            self.max_health[MP] += value
        elif ability == 3 and maxima:
            self.max_health[SP] += value
        elif ability == 4:
            stat(STR, None)
        elif ability == 5:
            stat(REC, HP)
        elif ability == 6:
            stat(INT, None)
        elif ability == 7:
            stat(WIS, MP)
        elif ability == 8:
            stat(DEX, SP)
        elif ability == 9:
            stat(LUC, None)
        elif ability in (10, 11, 12):
            self.sit_recovery_percent[{10: HP, 11: SP, 12: MP}[ability]] += value
        elif ability in (13, 14):
            self.cost_reduction[ability - 13] += value
        elif ability == 15:
            self.attack_range += value
        elif ability == 16:
            self.attack_speed += value
        elif ability == 17:
            self.move_speed += value
        elif ability == 18:
            self.critical += value
        elif ability == 19:
            self.cool_time += value
        elif 20 <= ability <= 22:
            self.adds[ability - 20].hit += value
        elif 23 <= ability <= 25:
            self.adds[ability - 23].attack += value
        elif 26 <= ability <= 28:
            self.adds[ability - 26].defence += value
        elif 29 <= ability <= 31:
            self.adds[ability - 29].evasion += value
        elif 32 <= ability <= 34:
            self.disabled[ability - 32] += value

    def plus(self, other: "Bonuses") -> "Bonuses":
        """`Bonuses::plus`."""
        b = Bonuses()
        for name in ("max_health", "stats", "sit_recovery_percent", "cost_reduction", "disabled"):
            setattr(b, name, [x + y for x, y in zip(getattr(self, name), getattr(other, name))])
        for name in ("attack_range", "attack_speed", "move_speed", "critical", "cool_time"):
            setattr(b, name, getattr(self, name) + getattr(other, name))
        b.adds = [Numbers(a.hit + o.hit, a.evasion + o.evasion, a.attack + o.attack, a.defence + o.defence)
                  for a, o in zip(self.adds, other.adds)]
        return b


@dataclass
class Passives:
    """`skills::Passives` (`CUser::AddSkillPassive`)."""
    weapon_damage: list = field(default_factory=lambda: [0] * WEAPON_TYPES)
    weapon_speed: list = field(default_factory=lambda: [0] * WEAPON_TYPES)
    shield: int = 0
    cross_road: int = 0
    bonuses: Bonuses = field(default_factory=Bonuses)
    low_hp: SkillDef | None = None

    @staticmethod
    def weapon(values, real: int) -> int:
        return values[real] if 0 <= real < len(values) else 0


def passives(defs: list) -> Passives:
    """`skills::passives` from the learnt skills' definitions (kind 0 only); also keeps the low-HP passive (`wType` 6)."""
    p = Passives()
    for d in defs:
        if d.type_effect != PASSIVE:
            continue
        weapons = [w for w in d.weapons if w != 0 and w < WEAPON_TYPES]
        if d.kind == 1:
            for w in weapons:
                p.weapon_damage[w] += d.weapon_value
        elif d.kind == 4:
            for ability, value in d.abilities:
                p.bonuses.add(ability, value, d.type_effect)
        elif d.kind == 8:
            for w in weapons:
                p.weapon_speed[w] += d.weapon_value
        elif d.kind == 9:
            p.cross_road += 1
        elif d.kind == 10:
            p.shield += d.weapon_value
        elif d.kind == LOW_HP:
            p.low_hp = d
    return p


def buff_bonuses(defs: list) -> Bonuses:
    """`PlayerBuffs::effects().bonuses` for applied skills: `wType` 82 and 91 buffs and an active low-HP passive add, monster or
    player debuffs of `wType` 62 and 67 take off (with `type_effect` 3, so maxima stay)."""
    b = Bonuses()
    for d in defs:
        if (d.type_effect, d.kind) in ((BUFF, ABILITIES), (BUFF, ABILITIES_TOO), (PASSIVE, LOW_HP)):
            sign = 1
        elif (d.type_effect, d.kind) in ((DEBUFF, MOB_ABILITIES), (DEBUFF, MOB_ABILITIES_TOO)):
            sign = -1
        else:
            continue
        for ability, value in d.abilities:
            b.add(ability, sign * value, d.type_effect)
    return b


def with_buffs(p: Passives, b: Bonuses) -> Passives:
    """`server::with_buffs`."""
    return replace(p, bonuses=p.bonuses.plus(b))


# --- lapis and enchant steps (lapis.rs) ------------------------------------------------------------------------------------------

LAPIS_TYPE = 30
CLOAK_SLOT = 7
ENCHANT_ADD = (7, 5)
ENCHANT_RATES = [(700_000, 990_000), (500_000, 980_000), (300_000, 970_000), (90_000, 300_000), (30_000, 100_000), (10_000, 30_000),
                 (3_000, 10_000), (1_000, 3_000), (300, 1_000), (100, 300), (30, 100), (10, 30), (1, 10), (1, 3)] + [(1, 1)] * 6


def enchant_value(craft_name: bytes) -> int:
    """`lapis::enchant_value`: the tenth two-digit number of a 20-character craft name."""
    if len(craft_name) == 20 and chr(craft_name[18]).isdigit() and chr(craft_name[19]).isdigit():
        return (craft_name[18] - 48) * 10 + (craft_name[19] - 48)
    return 0


def enchant_bonus(craft_name: bytes) -> tuple[int, int]:
    """`lapis::enchant_bonus`: (attack, absorption); 1 to 20 a weapon step, 51 to 70 an armour step."""
    v = enchant_value(craft_name)
    if 1 <= v <= 20:
        return v * ENCHANT_ADD[0], 0
    if 51 <= v <= 70:
        return 0, (v - 50) * ENCHANT_ADD[1]
    return 0, 0


def craft_values(craft_name: bytes):
    """`stats::craft_values`: STR, DEX, REC, INT, WIS, LUC, HP, MP, SP of a 20-character craft name."""
    if len(craft_name) != 20:
        return None
    values = []
    for i in range(9):
        a, b = craft_name[2 * i], craft_name[2 * i + 1]
        values.append((a - 48) * 10 + (b - 48) if 48 <= a <= 57 and 48 <= b <= 57 else 0)
    return values


def craft_name(stats=(0,) * 9, enchant: int = 0) -> bytes:
    """A craft name from nine crafted values and an enchant level (weapon step 1 to 20, armour step written + 50)."""
    return ("".join(f"{v:02d}" for v in stats) + f"{enchant:02d}").encode()


def lapis_of(defs: ItemDefs, gem: int):
    return defs.get(LAPIS_TYPE, gem) if gem else None


def element(base: int, gems, defs: ItemDefs) -> int:
    """`lapis::element`."""
    e = base
    for gem in gems:
        lap = lapis_of(defs, gem)
        if lap is None or lap.attrib == 0:
            continue
        if e == 0:
            e = lap.attrib
        elif e != lap.attrib:
            return 0
    return e


def equipped_lapis(item: ItemUnit, defs: ItemDefs):
    """`lapis::equipped_lapis`: not the cloak's badge."""
    if item.bag != 0 or item.slot == CLOAK_SLOT:
        return []
    return [lap for lap in (lapis_of(defs, g) for g in item.gems) if lap is not None]


def state_block(lap: ItemDef, slot: int):
    return lap.special if slot != 5 and 1 <= lap.special < 16 else None


# --- kill-count grade (kc.rs) ----------------------------------------------------------------------------------------------------

KC_TYPES = 28


def percent(value: int, pct: int) -> int:
    """`kc::percent`: `trunc(f32(value × pct) × 0.01f)`."""
    product = max(-2147483648, min(2147483647, value * pct))
    return i32(f32(product) * F_0_01)


# --- character numbers (stats.rs) ------------------------------------------------------------------------------------------------

BASE_MOVE_SPEED = 2
MAX_ATTACK_SPEED = 9
MAX_MOVE_SPEED = 4
STAT_HEALTH = 5
CRAFT_HEALTH = 100
RECOVERY_LAPIS = 16
WEAPON_SLOT = 5
ACCESSORY_SLOTS = range(8, 13)
COUNTED_SLOTS = 13


class StatusTable:
    """`stats::StatusTable`: `(job, level) -> [hp, sp, mp]`; outside, job 0 level 60."""

    def __init__(self, rows):
        self.rows = {(job, level): (hp, sp, mp) for job, level, hp, sp, mp in rows}

    def row(self, job: int, level: int):
        return self.rows.get((job, level)) or self.rows.get((0, 60)) or (0, 0, 0)


def scaled(value: int, grow: int) -> int:
    """`stats::scaled`: × 1.1f in hard mode, × 1.5f in ultimate, truncated."""
    if grow == 2:
        return i32(value * F_1_1)
    if grow == 3:
        return i32(value * F_1_5)
    return value


@dataclass
class Base:
    job: int
    level: int
    grow: int
    family: int
    stats: tuple
    grade: tuple = (0,) * KC_TYPES


@dataclass
class Totals:
    stats: list
    max_health: list  # HP, MP, SP
    attack_speed: int
    move_speed: int
    grade_recovery: list
    grade_fight_recovery: list

    def speeds(self):
        return clamp(self.attack_speed, 0, MAX_ATTACK_SPEED), clamp(self.move_speed, 0, MAX_MOVE_SPEED)


def compute(table: StatusTable, base: Base, items, defs: ItemDefs, p: Passives) -> Totals:
    """`stats::compute` (`CUser::InitEquipment`, `InitKCStatus`, `InitPassive`)."""
    hp, sp, mp = table.row(base.job, base.level)
    t = Totals(list(base.stats), [scaled(hp, base.grow), scaled(mp, base.grow), scaled(sp, base.grow)], 0, BASE_MOVE_SPEED,
               [0, 0, 0], [0, 0, 0])
    if base.family == 0:
        t.max_health[SP] += 10
    elif base.family == 1:
        t.max_health[MP] += 10
    elif base.family == 2:
        t.max_health[SP] += 5
        t.max_health[MP] += 5
    else:
        t.max_health[HP] += 10
    t.max_health[HP] += base.stats[REC] * STAT_HEALTH
    t.max_health[SP] += base.stats[DEX] * STAT_HEALTH
    t.max_health[MP] += base.stats[WIS] * STAT_HEALTH

    def add_stats(stats):
        for i in range(6):
            t.stats[i] += stats[i]
        t.max_health[HP] += stats[REC] * STAT_HEALTH
        t.max_health[SP] += stats[DEX] * STAT_HEALTH
        t.max_health[MP] += stats[WIS] * STAT_HEALTH

    for item in items:
        if item.bag != 0 or item.slot >= COUNTED_SLOTS:
            continue
        d = defs.get(item.item_type, item.type_id)
        if d is None:
            continue
        add_stats(d.const_stats)
        for i in range(3):
            t.max_health[i] += d.const_health[i]
        if item.slot == WEAPON_SLOT or item.slot in ACCESSORY_SLOTS:
            t.attack_speed += d.attack_time
        if item.slot == WEAPON_SLOT:
            t.attack_speed += Passives.weapon(p.weapon_speed, real_type(d.item_type))
        if item.slot in ACCESSORY_SLOTS:
            t.move_speed += d.speed
        for lap in equipped_lapis(item, defs):
            add_stats(lap.const_stats)
            if lap.special != RECOVERY_LAPIS:
                for i in range(3):
                    t.max_health[i] += lap.const_health[i]
            t.attack_speed += lap.attack_time
            t.move_speed += lap.speed
        if d.craftable:
            craft = craft_values(item.craft_name)
            if craft is not None:
                add_stats(craft[:6])
                t.max_health[HP] += craft[6] * CRAFT_HEALTH
                t.max_health[MP] += craft[7] * CRAFT_HEALTH
                t.max_health[SP] += craft[8] * CRAFT_HEALTH
    faction_grade(t, base.grade)
    b = p.bonuses
    for i in range(6):
        t.stats[i] += b.stats[i]
    for i in range(3):
        t.max_health[i] += b.max_health[i]
    t.attack_speed += b.attack_speed
    t.move_speed += b.move_speed
    return t


def faction_grade(t: Totals, g) -> None:
    """`stats::faction_grade`."""
    mx = list(t.max_health)
    t.max_health[HP] += percent(mx[HP], g[1])
    t.max_health[MP] += percent(mx[MP], g[2])
    t.max_health[SP] += percent(mx[SP], g[3])
    m = t.max_health
    t.grade_recovery = [percent(m[HP], g[10]), percent(m[MP], g[12]), percent(m[SP], g[11])]
    t.grade_fight_recovery = [percent(m[HP], g[13]), percent(m[MP], g[15]), percent(m[SP], g[14])]
    for i, kind in enumerate((4, 8, 5, 6, 7, 9)):
        t.stats[i] += g[kind]
    t.max_health[HP] += g[5] * STAT_HEALTH
    t.max_health[MP] += g[7] * STAT_HEALTH
    t.max_health[SP] += g[8] * STAT_HEALTH


# --- fighting numbers (combat.rs) ------------------------------------------------------------------------------------------------

ATTACK_INTERVALS_MS = [4000, 3000, 2750, 2500, 2250, 2000, 1750, 1500, 1250, 1000]
RANGED_REAL_TYPES = (11, 13, 14)
UPPER_ARMOUR_SLOT = 1
SHIELD_SLOT = 6

ELEMENTS = [
    [1.0, 0.8, 0.8, 0.8, 0.8, 0.7, 0.7, 0.7, 0.7],
    [1.2, 1.0, 0.5, 1.0, 1.4, 1.0, 0.4, 1.0, 1.3],
    [1.2, 1.4, 1.0, 0.5, 1.0, 1.3, 1.0, 0.4, 1.0],
    [1.2, 1.0, 1.4, 1.0, 0.5, 1.0, 1.3, 1.0, 0.4],
    [1.2, 0.5, 1.0, 1.4, 1.0, 0.4, 1.0, 1.3, 1.0],
    [1.3, 1.0, 0.5, 1.0, 1.6, 1.0, 0.5, 1.0, 1.4],
    [1.3, 1.6, 1.0, 0.5, 1.0, 1.4, 1.0, 0.5, 1.0],
    [1.3, 1.0, 1.6, 1.0, 0.5, 1.0, 1.4, 1.0, 0.5],
    [1.3, 0.5, 1.0, 1.6, 1.0, 0.5, 1.0, 1.4, 1.0],
]
ELEMENTS = [[f32(v) for v in row] for row in ELEMENTS]


def attack_interval_ms(attack_speed: int) -> int:
    """`combat::attack_interval`."""
    return ATTACK_INTERVALS_MS[min(max(attack_speed, 0), 9)]


def element_factor(attack: int, defence: int) -> float:
    """`combat::element`."""
    return ELEMENTS[attack][defence] if attack < 9 and defence < 9 else 1.0


@dataclass
class Gear:
    """`combat::Gear`."""
    attack: int = 0
    attack_var: int = 0
    defence: int = 0
    magic_defence: int = 0
    range: int = 0
    element: int = 0
    kind: int | None = None
    armour_element: int = 0
    absorption: int = 0
    state_blocks: int = 0
    adds: list = field(default_factory=_numbers3)
    critical: int = 0
    cross_road: bool = False
    defence_percent: tuple = (0, 0)


def wear_level(quality: int, full_quality: int) -> int:
    """`combat::wear_level`."""
    half = int(full_quality * 0.5)
    tenth = i32(full_quality * F_0_1)
    if quality > half:
        return 0
    if quality > tenth:
        return 1
    return 2


def worn(effect: int, quality: int, full_quality: int) -> int:
    level = wear_level(quality, full_quality)
    return effect if level == 0 else int(effect * 0.5) if level == 1 else 0


def gear(items, defs: ItemDefs, p: Passives) -> Gear:
    """`combat::gear` (`CUser::ItemEquipmentAdd`, `GemEquipmentAdd`, `AddSkillPassive`)."""
    b = p.bonuses
    g = Gear(range=b.attack_range, adds=[replace(a) for a in b.adds], critical=b.critical, cross_road=p.cross_road > 0)
    for item in items:
        if item.bag != 0 or item.slot >= COUNTED_SLOTS:
            continue
        d = defs.get(item.item_type, item.type_id)
        if d is None:
            continue
        e1, e2, e3, e4 = d.effects
        for lap in equipped_lapis(item, defs):
            g.range += lap.range
            a, v, df, m = lap.effects
            g.attack += a
            g.attack_var += v
            g.defence += df
            g.magic_defence += m
            g.absorption += lap.absorption
            s = state_block(lap, item.slot)
            if s is not None:
                g.state_blocks |= 1 << s
        if d.craftable:
            attack, absorption = enchant_bonus(item.craft_name)
            g.attack += attack
            g.absorption += absorption
        if item.slot == WEAPON_SLOT:
            g.range += d.range
            g.attack += worn(e1, item.quality, d.quality)
            g.attack_var += worn(e2, item.quality, d.quality)
            g.element = element(d.attrib, item.gems, defs)
            real = real_type(d.item_type)
            g.attack += Passives.weapon(p.weapon_damage, real)
            g.kind = RANGED if real in RANGED_REAL_TYPES else MELEE
        elif item.slot in ACCESSORY_SLOTS:
            g.range += d.range
            g.attack += e1
            g.attack_var += e2
            g.defence += e3
            g.magic_defence += e4
        else:
            if item.slot == UPPER_ARMOUR_SLOT:
                g.armour_element = element(d.attrib, item.gems, defs)
            if item.slot == SHIELD_SLOT:
                g.defence += p.shield
            g.defence += worn(e3, item.quality, d.quality)
            g.magic_defence += worn(e4, item.quality, d.quality)
    return g


def with_grade(g: Gear, grade) -> Gear:
    """`server::gear`'s faction-grade part: critical, melee hit, magic defence, evasions, defence percentages."""
    g = replace(g, adds=[replace(a) for a in g.adds])
    g.critical += grade[16]
    g.adds[0].hit += grade[17]
    g.adds[2].defence += grade[20]
    for add, kind in zip(g.adds, (21, 22, 23)):
        add.evasion += grade[kind]
    g.defence_percent = (grade[18], grade[19])
    return g


@dataclass
class Fighter:
    """`combat::Fighter`."""
    level: int = 0
    kinds: list = field(default_factory=_numbers3)
    critical: int = 0
    luc: int = 0
    wis: int = 0
    attack_var: int = 0
    element: int = 0
    size: int = 0
    int_: int = 0
    item_attack: int = 0


def player(level: int, stats, g: Gear) -> Fighter:
    """`combat::player` (`CUser::SetAttack`)."""
    s = [float(v) for v in stats]
    lv = float(level)
    dex_half = f32(s[DEX] * 0.5)
    dex_fifth = f32(s[DEX] * F_0_2)
    luc_evasion = f32(s[LUC] * 0.125 + dex_half)
    item_attack = float(g.attack)
    m, r, gm = ((float(a.hit), float(a.evasion), float(a.attack), a.defence) for a in g.adds)
    melee = Numbers(i32(m[0] + lv + dex_half), i32(m[1] + luc_evasion + lv), i32(s[STR] * F_1_3 + m[2] + item_attack + dex_fifth),
                    m[3] + stats[REC] + g.defence)
    ranged = Numbers(i32(r[0] + lv + dex_half), i32(r[1] + luc_evasion + lv),
                     i32(s[LUC] * F_0_3 + r[2] + item_attack + s[STR] + dex_fifth), r[3] + stats[REC] + g.defence)
    melee.defence += percent(melee.defence, g.defence_percent[0])
    ranged.defence += percent(ranged.defence, g.defence_percent[1])
    magic = Numbers(i32(gm[0]), i32(gm[1]), i32(s[INT] * F_1_3 + s[WIS] * F_0_2 + gm[2] + item_attack), gm[3] + g.magic_defence + stats[WIS])
    return Fighter(level, [melee, ranged, magic], i32(s[LUC] * F_0_19 + g.critical), stats[LUC], stats[WIS], g.attack_var, g.element, 0,
                   stats[INT], g.attack)


@dataclass
class MobAttack:
    skill: int = 0
    ok: int = 1
    time_ms: int = 0
    range: int = 0
    damage: int = 0
    damage_add: int = 0


@dataclass
class MobDef:
    """`mobs::MobDef`, the fighting fields."""
    id: int = 0
    level: int = 1
    hp: int = 0
    sp: int = 0
    mp: int = 0
    exp: int = 0
    dex: int = 0
    luc: int = 0
    wis: int = 0
    defense: int = 0
    magic: int = 0
    attrib: int = 0
    size: int = 0
    defense_add: int = 0
    country: int = 2
    resist_states: tuple = (False,) * 15
    resist_skills: tuple = (0,) * 6
    attacks: tuple = (MobAttack(), MobAttack(), MobAttack())
    recover: int = 0
    ai: int = 0
    name: str = ""


def monster(d: MobDef) -> Fighter:
    """`combat::monster` (`CMob::SetAttack`)."""
    dex, luc, lv = float(d.dex), float(d.luc), float(d.level)
    dex_half = dex * 0.5
    hit = i32(dex_half + lv)
    return Fighter(d.level, [Numbers(hit, i32(dex_half + lv / 2.0 + luc * 0.125), 0, d.defense),
                             Numbers(hit, i32(dex_half + luc * 0.125 + lv), 0, d.defense_add + d.defense),
                             Numbers(0, 0, d.wis + d.level, d.magic)],
                   i32(luc * F_0_19), d.luc, d.wis, 0, d.attrib, d.size)


@dataclass
class MobEffects:
    """`buffs::MobEffects`."""
    adds: list = field(default_factory=_numbers3)
    wis: int = 0
    dex: int = 0
    luc: int = 0
    no_element: bool = False


def mob_effects(debuffs) -> MobEffects:
    """`buffs::mob_effects` (the abilities part)."""
    e = MobEffects()
    for d in debuffs:
        if d.kind in (MOB_ABILITIES, MOB_ABILITIES_TOO):
            for ability, value in d.abilities:
                v = -value
                if ability == 7:
                    e.wis += v
                elif ability == 8:
                    e.dex += v
                elif ability == 9:
                    e.luc += v
                elif 20 <= ability <= 22:
                    e.adds[ability - 20].hit += v
                elif 23 <= ability <= 25:
                    e.adds[ability - 23].attack += v
                elif 26 <= ability <= 28:
                    e.adds[ability - 26].defence += v
                elif 29 <= ability <= 31:
                    e.adds[ability - 29].evasion += v
        elif d.kind == NO_ELEMENT:
            e.no_element = True
    return e


def monster_with(d: MobDef, e: MobEffects) -> Fighter:
    """`combat::monster_with`."""
    f = monster(d)
    for n, add in zip(f.kinds, e.adds):
        n.hit += add.hit
        n.evasion += add.evasion
        n.attack += add.attack
        n.defence += add.defence
    f.kinds[MAGIC].attack += e.wis
    f.critical = i32((d.luc + e.luc) * F_0_19)
    if e.no_element:
        f.element = 0
    return f


# --- players against monsters ------------------------------------------------------------------------------------------------------


def _rate(level_term: float, theirs: float, ours: float) -> float:
    return level_term - (theirs - ours)


def _div(a: float, b: float) -> float:
    """Float division as Rust does it: x/0 is ±inf, 0/0 NaN."""
    if b == 0:
        return math.nan if a == 0 else math.copysign(math.inf, a)
    return a / b


def hit_rate(p: Fighter, m: Fighter, kind: int) -> int:
    """`combat::hit_rate` (`CUser::AttackSuccess`), 20 to 99."""
    pk, mk = p.kinds[kind], m.kinds[kind]
    lu, lm = float(p.level), float(m.level)
    level = _div(lu, lu + lm) * 160.0
    theirs = _div(float(mk.hit), float(pk.evasion + mk.hit)) * 100.0
    ours = _div(float(pk.hit), float(mk.evasion + pk.hit)) * 100.0
    return clamp(i32(_rate(level, theirs, ours)), 20, 99)


def level_factor(attacker_level: int, defender_level: int) -> float:
    """P of `combat::formula` and `mob_strike`: `clamp(f32(((Ld − La − 1) × 100 + Ld) / 100 × 10), 1, 95)`; Ld is the monster's
    level whichever side attacks (the player's for a monster's blow is La)."""
    la, ld = float(attacker_level), float(defender_level)
    return fclamp(f32(((ld - la - 1.0) * 100.0 + ld) / 100.0 * 10.0), 1.0, 95.0)


def formula(p: Fighter, m: Fighter, kind: int, close: bool, bonus: int, elem: float, rand) -> tuple[int, int]:
    """`combat::formula` (`CUser::AttackDamage`)."""
    level = level_factor(p.level, m.level)
    attack = f32(float(p.kinds[kind].attack + bonus))
    if p.attack_var > 0:
        attack = f32(attack + float(rand() % p.attack_var))
    short = kind == RANGED and close
    if short:
        attack = f32(attack * F_0_7)

    def luck():
        return float(rand() % p.luc + 1) if p.luc > 0 else 0.0

    r = luck()
    defence = float(m.kinds[kind].defence)
    damage = (attack - attack / 100.0 * level) * elem - (defence + defence / 100.0 * level) + r * F_0_1
    damage = fmax(f32(damage), 0.0)
    critical = clamp(i32(float(p.critical) / 100.0 * 100.0 - float(m.luc) * F_0_034), 5, 99)
    result = RESULT_HIT
    if rand() % 100 < critical:
        r2 = f32(luck() * F_1_4)
        damage = f32(damage * 1.5 + r2 - r2 / 100.0 * level)
        if short:
            damage = f32(damage * F_0_7)
        result = RESULT_CRITICAL
    return min(i32(damage), MAX_DAMAGE), result


def at_least_one_to_ten(damage: int, rand) -> int:
    return rand() % 10 + 1 if damage <= 10 else damage


def strike(p: Fighter, m: Fighter, kind: int, close: bool, rand):
    """`combat::strike`: (result, damage)."""
    if rand() % 100 >= hit_rate(p, m, kind):
        return RESULT_MISS, 0
    damage, result = formula(p, m, kind, close, 0, element_factor(p.element, m.element), rand)
    return result, at_least_one_to_ten(damage, rand)


def skill_hits(p: Fighter, m: Fighter, d: MobDef, skill: SkillDef, rand) -> bool:
    """`combat::skill_hits`."""
    st = skill.state_type
    resists_state = st != 0 and st - 1 < len(d.resist_states) and d.resist_states[st - 1]
    if resists_state or skill.id in d.resist_skills:
        return False

    def against(ours, theirs):
        return 100 * (ours // (theirs + ours)) if ours + theirs != 0 else 0

    kind = min(skill.attack_kind, MAGIC)
    if skill.success_type == 1:
        return rand() % 100 < skill.success_value
    if skill.success_type == 2:
        return rand() % 100 < skill.success_value + against(p.luc, m.luc)
    if skill.success_type == 3:
        return rand() % 100 < skill.success_value + against(p.wis, m.wis)
    if kind == MAGIC:
        return rand() % 100 >= magic_resist_rate(p, m)
    return rand() % 100 < hit_rate(p, m, kind)


def magic_resist_rate(p: Fighter, m: Fighter) -> int:
    """`combat::magic_resist_rate`, 1 to 90."""
    lu, lm = float(p.level), float(m.level)
    wu, wm = float(p.wis), float(m.wis)
    a = fclamp(_div((lm - lu - 2.0) * 100.0 + lm, lu + lm) * 1.1 + float(m.kinds[MAGIC].evasion), 1.0, 70.0)
    b = fclamp(_div(wm * 11.0 - wu * 10.0, wu + wm) * 3.9 + float(p.kinds[MAGIC].hit), 1.0, 70.0)
    return clamp(i32(a + b), 1, 90)


def _capped(v: int) -> int:
    return clamp(v, 0, MAX_DAMAGE)


def skill_damage(p: Fighter, m: Fighter, d: MobDef, mob_health, caster, skill: SkillDef, close: bool, rand):
    """`combat::skill_damage`: (result, [HP, SP, MP]). `mob_health` and `caster` are current HP, SP, MP."""
    w = skill.damage

    def each(base, f):
        return [_capped(max(f(w[i], base[i]), 1)) if w[i] != 0 else 0 for i in range(3)]

    def pct(wv, of):
        return int(wv / 100.0 * of)

    if skill.damage_type == 0:
        return RESULT_HIT, list(w)
    if skill.damage_type == 3:
        return RESULT_HIT, each(caster, lambda a, b: a * b)
    if skill.damage_type == 4:
        return RESULT_HIT, each([d.hp, d.sp, d.mp], pct)
    if skill.damage_type == 5:
        return RESULT_HIT, each(mob_health, pct)
    kind = min(skill.attack_kind, MAGIC)
    attack_element = skill.element if skill.element != 0 else p.element
    factor = element_factor(attack_element, 0 if skill.kind == NEUTRAL_ELEMENT else m.element)
    if skill.damage_type == 2:
        dmg, result = formula(p, m, kind, close, 0, factor, rand)
        base = [dmg, 0, 0]
        damage = [base[i] * w[i] if w[i] != 0 else base[i] for i in range(3)]
    else:
        dmg, result = formula(p, m, kind, close, skill.damage[0], factor, rand)
        damage = [dmg, w[1], w[2]]
    damage = [_capped(v) for v in damage]
    damage[0] = at_least_one_to_ten(damage[0], rand)
    return result, damage


# --- monsters against players ------------------------------------------------------------------------------------------------------


def mob_hit_rate(m: Fighter, p: Fighter, kind: int) -> int:
    """`combat::mob_hit_rate` (`CMob::AttackSuccess`), 5 to 95."""
    mk, pk = m.kinds[kind], p.kinds[kind]
    lu, lm = float(p.level), float(m.level)
    level = _div(lm, lu + lm) * 160.0
    ours = _div(float(pk.hit), float(mk.evasion + pk.hit)) * 100.0
    theirs = _div(float(mk.hit), float(pk.evasion + mk.hit)) * 100.0
    return clamp(i32(level - (ours - theirs)), 5, 95)


def mob_strike(m: Fighter, damage: int, damage_add: int, strength: float, p: Fighter, defence_element: int, rand):
    """`combat::mob_strike` (`CMob::Attack`, `CMob::AttackDamage`): (result, damage)."""
    if rand() % 100 >= mob_hit_rate(m, p, MELEE):
        return RESULT_MISS, 0
    level = level_factor(p.level, m.level)
    attack = f32(f32(float(damage) * strength) + float(m.kinds[MELEE].attack))
    if damage_add > 0:
        attack = f32(attack + float(rand() % damage_add))
    elem = ELEMENTS[0][defence_element] if defence_element < 9 else 1.0

    def luck():
        return float(rand() % m.luc + 1) if m.luc > 0 else 0.0

    r = luck()
    defence = float(p.kinds[MELEE].defence)
    hurt = (attack + attack / 100.0 * level) * elem - (defence - defence / 100.0 * level) + r * F_0_1
    hurt = fmax(f32(hurt), 0.0)
    critical = clamp(i32(float(m.critical) / 100.0 * 100.0 - float(p.luc) * F_0_034), 5, 99)
    result = RESULT_HIT
    if rand() % 100 < critical:
        r2 = f32(luck() * F_1_4)
        hurt = f32(r2 / 100.0 * level + hurt * 1.5 + r2)
        result = RESULT_CRITICAL
    out = min(i32(hurt), MAX_DAMAGE)
    if out <= 10:
        out = rand() % 10 + 1
    return result, out


def mob_skill_hits(m: Fighter, d: MobDef, p: Fighter, skill: SkillDef, rand) -> bool:
    """`combat::mob_skill_hits`."""
    def against(current, base):
        return 100 * (current // (base + current)) if current + base != 0 else 0

    kind = min(skill.attack_kind, MAGIC)
    roll = rand() % 100
    if skill.success_type == 1:
        return roll < skill.success_value
    if skill.success_type == 2:
        return roll < skill.success_value + against(m.luc, d.luc)
    if skill.success_type == 3:
        return roll < skill.success_value + against(m.wis, d.wis)
    if kind == MAGIC:
        return roll >= player_resist_rate(m, d, p)
    return roll < mob_hit_rate(m, p, kind)


def player_resist_rate(m: Fighter, d: MobDef, p: Fighter) -> int:
    """`combat::player_resist_rate`, 1 to 90."""
    lu, lm = float(p.level), float(m.level)
    wu, wm = float(p.wis), float(d.wis)
    a = fclamp(_div((lu - lm - 2.0) * 100.0 + lu, lu + lm) * 1.1 + float(p.kinds[MAGIC].evasion), 1.0, 70.0)
    b = fclamp(_div(wu * 11.0 - wm * 10.0, wu + wm) * 3.9 + float(m.kinds[MAGIC].hit), 1.0, 70.0)
    return clamp(i32(a + b), 1, 90)


def mob_skill_damage(m: Fighter, entry_damage: int, entry_damage_add: int, mob_health, p: Fighter, defence_element: int, health,
                     mx, skill: SkillDef, rand):
    """`combat::mob_skill_damage`: (result, [HP, SP, MP]). `health`, `mx` the player's HP, SP, MP and maxima."""
    w = skill.damage

    def each(base, f):
        return [_capped(max(f(w[i], base[i]), 1)) if w[i] != 0 else 0 for i in range(3)]

    def pct(wv, of):
        return int(f32(f32(wv / 100.0) * of))

    if skill.damage_type == 0:
        return RESULT_HIT, list(w)
    if skill.damage_type == 3:
        return RESULT_HIT, each(mob_health, lambda a, b: a * b)
    if skill.damage_type == 4:
        return RESULT_HIT, each(mx, pct)
    if skill.damage_type == 5:
        return RESULT_HIT, each(health, pct)
    kind = min(skill.attack_kind, MAGIC)
    level = level_factor(p.level, m.level)
    bonus = skill.damage[0] if skill.damage_type == 1 else 0
    attack = f32(float(entry_damage + m.kinds[kind].attack + bonus))
    if entry_damage_add > 0:
        attack = f32(attack + float(rand() % entry_damage_add))
    factor = element_factor(skill.element, 0) if skill.kind == NEUTRAL_ELEMENT else element_factor(skill.element, defence_element)

    def luck():
        return float(rand() % m.luc + 1) if m.luc > 0 else 0.0

    r = luck()
    defence = float(p.kinds[kind].defence)
    hurt = (attack + attack / 100.0 * level) * factor - (defence - defence / 100.0 * level) + r * F_0_1
    hurt = fmax(f32(hurt), 0.0)
    critical = clamp(i32(float(m.critical) / 100.0 * 100.0 - float(p.luc) * F_0_034), 5, 99)
    result = RESULT_HIT
    if rand() % 100 < critical:
        r2 = f32(luck() * F_1_4 / 100.0 * level)
        hurt = f32(hurt * 1.5 + r2)
        result = RESULT_CRITICAL
    first = i32(hurt)
    if skill.damage_type == 1:
        damage = [first, w[1], w[2]]
    elif skill.damage_type == 2 and w[0] != 0:
        damage = [first * w[0], 0, 0]
    else:
        damage = [first, 0, 0]
    damage = [_capped(v) for v in damage]
    damage[0] = at_least_one_to_ten(damage[0], rand)
    return result, damage


# --- players against players -------------------------------------------------------------------------------------------------------


def pvp_hit_rate(a: Fighter, t: Fighter, kind: int) -> int:
    """`combat::pvp_hit_rate`, 1 to 99."""
    ak, tk = a.kinds[kind], t.kinds[kind]
    la, lt = float(a.level), float(t.level)
    level = _div(la, lt + la) * 160.0
    theirs = _div(float(tk.hit), float(ak.evasion + tk.hit)) * 100.0
    ours = _div(float(ak.hit), float(tk.evasion + ak.hit)) * 100.0
    return clamp(i32(_rate(level, theirs, ours)), 1, 99)


def pvp_strike(a: Fighter, t: Fighter, kind: int, close: bool, cross_road: bool, defence_element: int, rand):
    """`combat::pvp_strike`: (result, damage)."""
    if rand() % 100 >= pvp_hit_rate(a, t, kind):
        return RESULT_MISS, 0
    d = a.kinds[kind].attack - t.kinds[kind].defence
    if a.attack_var > 0:
        d += rand() % a.attack_var
    d = max(d, 0)
    if kind == RANGED and close and not cross_road:
        d = i32(d * F_0_7)
    d = i32(d * element_factor(a.element, defence_element))

    def luck():
        return rand() % a.luc + 1 if a.luc > 0 else 0

    r = luck()
    d = i32(r * F_0_1 + d)
    critical = clamp(i32(float(a.critical) / 100.0 * 100.0 - float(t.luc) * F_0_034), 1, 99)
    result = RESULT_HIT
    if rand() % 100 < critical:
        r2 = luck()
        d = i32(d * 0.5 + r2 * F_1_4 + d)
        if kind == RANGED and close:
            d = i32(d * F_0_7)
        result = RESULT_CRITICAL
    return result, at_least_one_to_ten(min(d, MAX_DAMAGE), rand)


def pvp_resist_rate(c: Fighter, t: Fighter) -> int:
    """`combat::pvp_resist_rate`, 1 to 90 (single precision as the Rust code)."""
    la, lt = float(c.level), float(t.level)
    wa, wt = float(c.wis), float(t.wis)
    a = fclamp(f32(f32(f32(_div((lt - la - 2.0) * 100.0 + lt, la + lt)) * F_1_1) + f32(float(t.kinds[MAGIC].evasion))), 1.0, 70.0)
    b = fclamp(f32(f32(f32(_div(wt * 11.0 - wa * 10.0, wa + wt)) * f32(3.9)) + f32(float(c.kinds[MAGIC].hit))), 1.0, 70.0)
    return clamp(i32(a + b), 1, 90)


def pvp_skill_hits(c: Fighter, t: Fighter, skill: SkillDef, rand) -> bool:
    """`combat::pvp_skill_hits` (after the block and state protections, which are the caller's)."""
    def against(ours, theirs):
        return 100 * (ours // (theirs + ours)) if ours + theirs != 0 else 0

    if skill.success_type == 1:
        return rand() % 100 < skill.success_value
    if skill.success_type == 2:
        return rand() % 100 < skill.success_value + against(c.luc, t.luc)
    if skill.success_type == 3:
        return rand() % 100 < skill.success_value + against(c.wis, t.wis)
    if skill.attack_kind == MAGIC:
        return rand() % 100 >= pvp_resist_rate(c, t)
    if skill.attack_kind in (MELEE, RANGED):
        return rand() % 100 < pvp_hit_rate(c, t, skill.attack_kind)
    return True


@dataclass
class PvpSkillHit:
    caster: Fighter
    target: Fighter
    defence_element: int = 0
    caster_health: tuple = (0, 0, 0)  # HP, SP, MP
    target_health: tuple = (0, 0, 0)
    target_max: tuple = (0, 0, 0)
    within_three: bool = False
    cross_road: bool = False


def pvp_magic_cut(c: Fighter, t: Fighter) -> tuple[float, float]:
    """The two percentage cuts of a magic skill on a player (`combat::pvp_skill_damage`): P1 the level gap (1 to 50), P2 the target's
    magic defence against the caster's INT + item attack (1 to 70)."""
    la, lt = float(c.level), float(t.level)
    p1 = fclamp(float(f32(f32(_div(101.0 * lt - 100.0 - 100.0 * la, la + lt)) * F_2_4)), 1.0, 50.0)
    num, den = f32(float(t.kinds[MAGIC].defence)), f32(float(c.item_attack + c.int_))
    p2 = float(fclamp(f32(f32(_div(num, den)) * 100.0), 1.0, 70.0))
    return p1, p2


def pvp_skill_damage(h: PvpSkillHit, skill: SkillDef, rand):
    """`combat::pvp_skill_damage`: (result, [HP, SP, MP])."""
    w = skill.damage

    def each(base, f):
        return [_capped(max(f(w[i], base[i]), 1)) if w[i] != 0 else 0 for i in range(3)]

    def pct(wv, of):
        return int(f32(f32(wv / 100.0) * of))

    if skill.damage_type == 0:
        return RESULT_HIT, list(w)
    if skill.damage_type == 3:
        return RESULT_HIT, each(h.caster_health, lambda a, b: a * b)
    if skill.damage_type == 4:
        return RESULT_HIT, each(h.target_max, pct)
    if skill.damage_type == 5:
        return RESULT_HIT, each(h.target_health, pct)
    a, t = h.caster, h.target
    attack_element = skill.element if skill.element != 0 else a.element
    factor = element_factor(attack_element, 0 if skill.kind == NEUTRAL_ELEMENT else h.defence_element)
    bonus = skill.damage[0] if skill.damage_type == 1 else 0

    def luck():
        return float(rand() % a.luc + 1) if a.luc > 0 else 0.0

    critical = clamp(i32(float(a.critical) / 100.0 * 100.0 - float(t.luc) * F_0_034), 1, 99)
    result = RESULT_HIT
    if skill.attack_kind in (MELEE, RANGED):
        kind = skill.attack_kind
        d = bonus + a.kinds[kind].attack - t.kinds[kind].defence
        if a.attack_var > 0:
            d += rand() % a.attack_var
        d = max(d, 0)
        if kind == RANGED and h.within_three and not h.cross_road:
            d = i32(d * F_0_7)
        d = i32(d * factor)
        d = i32(luck() * F_0_1 + d)
        if rand() % 100 < critical:
            d = i32(d * 0.5 + luck() * F_1_4 + d)
            if kind == RANGED and h.within_three:
                d = i32(d * F_0_7)
            result = RESULT_CRITICAL
        first = d
    elif skill.attack_kind == MAGIC:
        p1, p2 = pvp_magic_cut(a, t)
        x = float(f32(f32(factor) * f32(float(a.kinds[MAGIC].attack + bonus))))
        d = float(fmax(f32(x - x / 100.0 * (p1 + p2)), 0.0))
        d = float(f32(luck() * F_0_1 + d))
        if rand() % 100 < critical:
            d = float(f32(d * 0.5 + luck() * F_1_4 + d))
            result = RESULT_CRITICAL
        first = i32(d)
    else:
        first = bonus
    if skill.damage_type == 1:
        damage = [first, w[1], w[2]]
    elif skill.damage_type == 2 and w[0] != 0:
        damage = [first * w[0], 0, 0]
    else:
        damage = [first, 0, 0]
    damage = [_capped(v) for v in damage]
    damage[0] = at_least_one_to_ten(damage[0], rand)
    return result, damage


# --- what happens to the damage on a player ----------------------------------------------------------------------------------------


def absorb(damage: int, absorption: int) -> tuple[int, int]:
    """`combat::absorb`: (HP damage left, absorbed); at least 1 is left."""
    if damage == 0 or absorption <= 0:
        return damage, 0
    a = min(absorption, 65535)
    return (1, damage - 1) if damage <= a else (damage - a, a)


def give_up(mode: int, damage: int, health: list) -> list:
    """`buffs::give_up`: `health` HP, MP, SP (changed); returns [HP left, SP paid, MP paid]."""
    left, sp, mp = damage, 0, 0
    if mode in (1, 3):
        sp = min(left, health[2])
        health[2] -= sp
        left -= sp
    if mode in (2, 3):
        mp = min(left, health[1])
        health[1] -= mp
        left -= mp
    return [left, sp, mp]


# --- experience and levels ---------------------------------------------------------------------------------------------------------


def mob_exp(d: MobDef, level: int, rate: float = 1.0) -> int:
    """`combat::mob_exp` (`CMob::GetExp`)."""
    diff = d.level - level
    if diff > 9:
        return 0
    exp = int(d.exp * ((diff + 1) * 0.2 + 1.0))
    if rate != 1.0:
        exp = int(exp * rate)
    return max(exp, 0)


def kill_exp(d: MobDef, level: int, rate: float = 1.0) -> int:
    """`combat::kill_exp`: the killer's 1.2."""
    return int(mob_exp(d, level, rate) * 1.2)


def max_level(grow: int) -> int:
    return 30 if grow == 0 else 60


def class_stat(job: int):
    """`combat::class_stat`: fighter STR, defender REC, ranger DEX, archer LUC, mage INT, priest WIS."""
    return [STR, REC, DEX, LUC, INT, WIS][job] if 0 <= job < 6 else None


POINTS_PER_LEVEL = {0: (5, 3), 1: (5, 3), 2: (7, 4), 3: (9, 5)}  # (stat, skill) by mode


class ExpTable:
    """`enter::ExpTable`: from `defs.exp_defs` rows (level, grow 1 to 4, exp), running totals per mode 0 to 3."""

    def __init__(self, rows):
        self.totals = [[0] * 61 for _ in range(4)]
        for level, grow, exp in rows:
            mode = grow - 1
            if 1 <= level <= 60 and 0 <= mode < 4:
                self.totals[mode][level] = exp
        for mode in self.totals:
            for level in range(1, 61):
                mode[level] = (mode[level] + mode[level - 1]) & 0xFFFFFFFF

    def total(self, grow: int, level: int) -> int:
        if grow < 4 and level <= 60:
            return self.totals[grow][level]
        return self.totals[0][60]


@dataclass
class Progress:
    level: int
    exp: int
    stat_point: int
    skill_point: int
    stats: list


def add_exp(progress: Progress, job: int, grow: int, gain: int, table: ExpTable):
    """`combat::add_exp`: (progress, levels gained) or None."""
    mx = max_level(grow)
    if gain == 0 or progress.level >= mx:
        return None
    p = replace(progress, stats=list(progress.stats))
    exp = min(p.exp + gain, 0xFFFFFFFF)
    levels = 0
    while exp >= table.total(grow, p.level):
        p.level += 1
        levels += 1
        s = class_stat(job)
        if s is not None:
            p.stats[s] = min(p.stats[s] + 1, 65535)
        stat_points, skill_points = POINTS_PER_LEVEL[grow]
        p.stat_point += stat_points
        p.skill_point += skill_points
        if p.level >= mx:
            p.exp = table.total(grow, p.level - 1)
            return p, levels
    p.exp = exp
    return p, levels


# --- creation and resets (creation.rs, item_use.rs) --------------------------------------------------------------------------------

CLASS_STATS = [
    [14, 12, 9, 8, 7, 11],   # fighter / warrior
    [10, 9, 12, 10, 10, 10],  # defender / guardian
    [10, 15, 9, 7, 8, 12],   # ranger / assassin
    [11, 10, 10, 7, 10, 13],  # archer / hunter
    [7, 9, 9, 15, 12, 9],    # mage / pagan
    [8, 9, 10, 12, 14, 8],   # priest / oracle
]
RACE_CLASSES = [[0, 1, 5], [2, 3, 4], [2, 4, 5], [0, 1, 3]]  # human, elf, vail, deatheater


def starting_stats(job: int, family: int) -> list:
    """`creation::starting_stats`: human LUC + 4, elf DEX + 4, vail INT and WIS + 2, deatheater STR and REC + 2."""
    s = list(CLASS_STATS[min(job, 5)])
    if family == 0:
        s[LUC] += 4
    elif family == 1:
        s[DEX] += 4
    elif family == 2:
        s[INT] += 2
        s[WIS] += 2
    else:
        s[REC] += 2
        s[STR] += 2
    return s


def reset_stats(job: int, family: int, grow: int, level: int):
    """`item_use::reset_stats`: (stat points, base stats) of a fresh build at this level."""
    n = max(level - 1, 0)
    s = starting_stats(job, family)
    c = class_stat(job)
    if c is not None:
        s[c] += n
    return n * POINTS_PER_LEVEL[grow][0], s


def reset_skill_points(grow: int, level: int) -> int:
    """`item_use::reset_skill_points`."""
    return max(level - 1, 0) * POINTS_PER_LEVEL[grow][1] + 5


# --- recovery (zone.rs `Zones::recover`) -------------------------------------------------------------------------------------------

REC_NORMAL_AMOUNT = 4
REC_SITTING_AMOUNT = 28
RECOVER_EVERY_MS = 5000


def recovery(state: str, max_value: int, sit_percent: int = 0, grade: int = 0, fighting: int = 0) -> int:
    """What one recovery tick (every 5 s) gives one of HP, MP, SP: sitting `trunc(p % + 5 % of the maximum + 28 + grade)`, standing
    `4 + grade`, fighting the grade's fight recovery only."""
    if state == "sitting":
        mx = float(max_value)
        return i32(float(sit_percent) * mx * F_0_01 + mx * F_0_05 + float(REC_SITTING_AMOUNT + grade))
    if state == "normal":
        return REC_NORMAL_AMOUNT + grade
    return fighting
