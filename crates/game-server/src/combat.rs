//! Players fighting monsters with normal attacks (`docs/original/combat.md`) and skills (`docs/original/skills-use.md` sections 11
//! and 12): the attack numbers, range, hit chance, damage, experience and level-up. Pure functions; the zone and the connection apply
//! them.
//!
//! The original computes on the x87 and stores some intermediate values as floats; this computes in double precision and rounds to
//! single precision where the original stores, so results match except in rare cases right at a whole number.

use std::time::Duration;

use crate::backend::ItemUnit;
use crate::enter::ExpTable;
use crate::inventory::{ItemDefs, real_type};
use crate::lapis;
use crate::mobs::MobDef;
use crate::skills::{self, Passives, SkillDef};

pub const MELEE: usize = 0;
pub const RANGED: usize = 1;
pub const MAGIC: usize = 2;

/// `0x0503`/`0x0502` results.
pub const RESULT_HIT: u8 = 0;
pub const RESULT_CRITICAL: u8 = 1;
pub const RESULT_MISS: u8 = 3;
pub const RESULT_OUT_OF_RANGE: u8 = 5;
pub const RESULT_NO_WEAPON: u8 = 7;
pub const RESULT_CANNOT_ATTACK: u8 = 10;

/// Wait between normal attacks by attack speed 0 to 9 (`g_ConstUserAttackSpeed`).
const ATTACK_INTERVALS_MS: [u64; 10] = [4000, 3000, 2750, 2500, 2250, 2000, 1750, 1500, 1250, 1000];
/// `g_ConstUserMoveSpeed` by move speed 0 to 4.
const MOVE_SLACK: [f32; 5] = [3.0, 4.0, 5.0, 6.0, 7.0];
/// Weapons of these real types attack from range (`CUser::AttackMob`).
const RANGED_REAL_TYPES: [u8; 3] = [11, 13, 14];
const STR: usize = 0;
const DEX: usize = 1;
const REC: usize = 2;
const INT: usize = 3;
const WIS: usize = 4;
const LUC: usize = 5;
const WEAPON_SLOT: u8 = 5;
const UPPER_ARMOUR_SLOT: u8 = 1;
const SHIELD_SLOT: u8 = 6;
const ACCESSORY_SLOTS: std::ops::RangeInclusive<u8> = 8..=12;
/// Equipment slots that count (not the mount in 13).
const COUNTED_SLOTS: u8 = 13;
/// Most damage one blow does (`CUser::AttackDamage`).
const MAX_DAMAGE: i32 = 30_000;

/// Element factors (`CGameData::m_AttackAttr`): rows the attacker's element, columns the defender's.
const ELEMENTS: [[f32; 9]; 9] = [
    [1.0, 0.8, 0.8, 0.8, 0.8, 0.7, 0.7, 0.7, 0.7],
    [1.2, 1.0, 0.5, 1.0, 1.4, 1.0, 0.4, 1.0, 1.3],
    [1.2, 1.4, 1.0, 0.5, 1.0, 1.3, 1.0, 0.4, 1.0],
    [1.2, 1.0, 1.4, 1.0, 0.5, 1.0, 1.3, 1.0, 0.4],
    [1.2, 0.5, 1.0, 1.4, 1.0, 0.4, 1.0, 1.3, 1.0],
    [1.3, 1.0, 0.5, 1.0, 1.6, 1.0, 0.5, 1.0, 1.4],
    [1.3, 1.6, 1.0, 0.5, 1.0, 1.4, 1.0, 0.5, 1.0],
    [1.3, 1.0, 1.6, 1.0, 0.5, 1.0, 1.4, 1.0, 0.5],
    [1.3, 0.5, 1.0, 1.6, 1.0, 0.5, 1.0, 1.4, 1.0],
];

/// The wait after a normal attack (`CUser::UpdateAttack`).
pub fn attack_interval(attack_speed: u8) -> Duration {
    Duration::from_millis(ATTACK_INTERVALS_MS[usize::from(attack_speed.min(9))])
}

/// The slack for a player who may have moved since the server last heard (`CUser::GetAttackEnableRange`): three times the move
/// speed's value, scaled down when the last move was within a second, at least 2.
pub fn range_slack(move_speed: u8, since_move: Option<Duration>) -> f32 {
    let full = MOVE_SLACK[usize::from(move_speed.min(4))] * 3.0;
    let slack = match since_move {
        Some(elapsed) if elapsed <= Duration::from_secs(1) => elapsed.as_millis() as f32 / 1000.0 * full,
        _ => full,
    };
    slack.max(2.0)
}

/// Whether a monster is within reach (`CUser::EnableAttackRange`).
pub fn in_range(distance: f32, slack: f32, mob_size: u8, player_range: i32) -> bool {
    distance <= slack + f32::from(mob_size) + player_range as f32 + 2.3
}

/// One kind's numbers (`stAttack`, `stAttackMob`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Numbers {
    pub hit: i32,
    pub evasion: i32,
    pub attack: i32,
    pub defence: i32,
}

/// A side of a fight: level, the numbers for melee, ranged and magic, critical, luck, wisdom (magic skills), the random attack
/// spread, the element and the size (monsters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fighter {
    pub level: u16,
    pub kinds: [Numbers; 3],
    pub critical: i32,
    pub luc: i32,
    pub wis: i32,
    pub attack_var: i32,
    pub element: u8,
    pub size: u8,
    /// INT and the attack the equipment adds (`m_nItemAttack`): a magic skill on a player is cut by the target's magic defence against
    /// their sum (`docs/original/pvp-2.md` section 6). 0 for monsters.
    pub int: i32,
    pub item_attack: i32,
}

/// What the equipment and passive skills add to the attack numbers (`CUser::ItemEquipmentAdd`, `CUser::AddSkillPassive`), and the
/// weapon's attack kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gear {
    pub attack: i32,
    pub attack_var: i32,
    pub defence: i32,
    pub magic_defence: i32,
    pub range: i32,
    pub element: u8,
    /// `None` without a weapon in slot 5.
    pub kind: Option<usize>,
    /// The upper armour's element with its lapis, the player's defence element against monsters (`CUser::SetAttrDef`).
    pub armour_element: u8,
    /// Absorption from lapis and armour enchant steps (`m_nReduceDamage`, `docs/original/lapis.md` section 2).
    pub absorption: i32,
    /// The states lapis block, a bit per state 1 to 15 (`m_pStateBlock`).
    pub state_blocks: u16,
    /// The `add` terms of the melee, ranged and magic numbers, and the critical bonus (`m_pAttackAdd`, `m_nCritical`).
    pub adds: [Numbers; 3],
    pub critical: i32,
    /// Ranged attacks within 3 keep their full damage (`m_nCrossRoad`).
    pub cross_road: bool,
    /// The faction grade's melee and ranged defence in percent of that defence (types 18 and 19, `docs/original/pvp-2.md`
    /// section 8).
    pub defence_percent: [i32; 2],
}

/// HP damage after absorption (`CUser::Damage`): at least 1 is left; returns the damage and what was absorbed.
pub fn absorb(damage: u16, absorption: i32) -> (u16, u16) {
    if damage == 0 || absorption <= 0 {
        return (damage, 0);
    }
    let a = absorption.min(i32::from(u16::MAX)) as u16;
    if damage <= a { (1, damage - 1) } else { (damage - a, a) }
}

/// An item's wear level (`CItem::GetQualityLevel`, `docs/original/repair.md` section 1): 0 while its quality is above half the
/// definition's, 1 while above a tenth, else 2 (`CGameData::LoadConstItem`: `trunc(Q × 0.5f)`, `trunc(Q × 0.1f)`).
pub fn wear_level(quality: u16, full_quality: u16) -> u8 {
    let half = (f64::from(full_quality) * 0.5) as u16;
    let tenth = (f64::from(full_quality) * f64::from(0.1f32)) as u16;
    if quality > half {
        0
    } else if quality > tenth {
        1
    } else {
        2
    }
}

/// The effect at an item's wear level: full, half, then nothing (`CUser::ItemEquipmentAdd`).
fn worn(effect: u16, quality: u16, full_quality: u16) -> i32 {
    match wear_level(quality, full_quality) {
        0 => i32::from(effect),
        1 => (f64::from(effect) * 0.5) as i32,
        _ => 0,
    }
}

pub fn gear<'a>(items: impl IntoIterator<Item = &'a ItemUnit>, defs: &ItemDefs, passives: &Passives) -> Gear {
    let b = &passives.bonuses;
    let mut g = Gear { range: b.attack_range, adds: b.adds, critical: b.critical, cross_road: passives.cross_road > 0, ..Gear::default() };
    for item in items.into_iter().filter(|i| i.bag == 0 && i.slot < COUNTED_SLOTS) {
        let Some(def) = defs.get(item.item_type, item.type_id) else { continue };
        let [e1, e2, e3, e4] = def.effects;
        // Lapis (`CUser::GemEquipmentAdd`) and the enchant step (`CItem::CopyCraftExpassion`), whatever the quality.
        for l in lapis::equipped_lapis(item, defs) {
            g.range += i32::from(l.range);
            let [a, v, d, m] = l.effects.map(i32::from);
            (g.attack, g.attack_var, g.defence, g.magic_defence) = (g.attack + a, g.attack_var + v, g.defence + d, g.magic_defence + m);
            g.absorption += i32::from(l.absorption);
            if let Some(state) = lapis::state_block(l, item.slot) {
                g.state_blocks |= 1 << state;
            }
        }
        if def.craftable {
            let (attack, absorption) = lapis::enchant_bonus(&item.craft_name, defs.enchant());
            g.attack += attack;
            g.absorption += absorption;
        }
        if item.slot == WEAPON_SLOT {
            g.range += i32::from(def.range);
            g.attack += worn(e1, item.quality, def.quality);
            g.attack_var += worn(e2, item.quality, def.quality);
            g.element = lapis::element(def.attrib, &item.gems, defs);
            let real = real_type(def.item_type);
            g.attack += Passives::weapon(&passives.weapon_damage, real);
            g.kind = Some(if RANGED_REAL_TYPES.contains(&real) { RANGED } else { MELEE });
        } else if ACCESSORY_SLOTS.contains(&item.slot) {
            g.range += i32::from(def.range);
            g.attack += i32::from(e1);
            g.attack_var += i32::from(e2);
            g.defence += i32::from(e3);
            g.magic_defence += i32::from(e4);
        } else {
            if item.slot == UPPER_ARMOUR_SLOT {
                g.armour_element = lapis::element(def.attrib, &item.gems, defs);
            }
            if item.slot == SHIELD_SLOT {
                g.defence += passives.shield;
            }
            g.defence += worn(e3, item.quality, def.quality);
            g.magic_defence += worn(e4, item.quality, def.quality);
        }
    }
    g
}

/// A player's numbers (`CUser::SetAttack`) from its level, stat totals (STR, DEX, REC, INT, WIS, LUC) and gear (equipment and passive
/// skills; the `add` terms of `docs/original/combat.md` section 4).
pub fn player(level: u16, stats: [i32; 6], gear: &Gear) -> Fighter {
    let s = stats.map(f64::from);
    let l = f64::from(level);
    let f = |x: f64| f64::from(x as f32);
    let dex_half = f(s[DEX] * 0.5);
    let dex_fifth = f(s[DEX] * f64::from(0.2f32));
    let luc_evasion = f(s[LUC] * 0.125 + dex_half);
    let item_attack = f64::from(gear.attack);
    let [m, r, g] = gear.adds.map(|add| (f64::from(add.hit), f64::from(add.evasion), f64::from(add.attack), add.defence));
    let melee = Numbers {
        hit: (m.0 + l + dex_half) as i32,
        evasion: (m.1 + luc_evasion + l) as i32,
        attack: (s[STR] * f64::from(1.3f32) + m.2 + item_attack + dex_fifth) as i32,
        defence: m.3 + stats[REC] + gear.defence,
    };
    let ranged = Numbers {
        hit: (r.0 + l + dex_half) as i32,
        evasion: (r.1 + luc_evasion + l) as i32,
        attack: (s[LUC] * f64::from(0.3f32) + r.2 + item_attack + s[STR] + dex_fifth) as i32,
        defence: r.3 + stats[REC] + gear.defence,
    };
    let (mut melee, mut ranged) = (melee, ranged);
    melee.defence += crate::kc::percent(melee.defence, gear.defence_percent[0]);
    ranged.defence += crate::kc::percent(ranged.defence, gear.defence_percent[1]);
    let magic = Numbers {
        hit: g.0 as i32,
        evasion: g.1 as i32,
        // WIS × 0.2 is added unrounded here, unlike the DEX terms.
        attack: (s[INT] * f64::from(1.3f32) + s[WIS] * f64::from(0.2f32) + g.2 + item_attack) as i32,
        defence: g.3 + gear.magic_defence + stats[WIS],
    };
    Fighter {
        level,
        kinds: [melee, ranged, magic],
        critical: (s[LUC] * f64::from(0.19f32) + f64::from(gear.critical)) as i32,
        luc: stats[LUC],
        wis: stats[WIS],
        attack_var: gear.attack_var,
        element: gear.element,
        size: 0,
        int: stats[INT],
        item_attack: gear.attack,
    }
}

/// A monster's numbers (`CMob::SetAttack`), without buffs.
pub fn monster(def: &MobDef) -> Fighter {
    let (dex, luc, l) = (f64::from(def.dex), f64::from(def.luc), f64::from(def.level));
    let dex_half = dex * 0.5;
    let hit = (dex_half + l) as i32;
    Fighter {
        level: def.level,
        kinds: [
            Numbers { hit, evasion: (dex_half + l / 2.0 + luc * 0.125) as i32, attack: 0, defence: i32::from(def.defense) },
            Numbers {
                hit,
                evasion: (dex_half + luc * 0.125 + l) as i32,
                attack: 0,
                defence: def.defense_add as i32 + i32::from(def.defense),
            },
            Numbers { hit: 0, evasion: 0, attack: i32::from(def.wis) + i32::from(def.level), defence: i32::from(def.magic) },
        ],
        critical: (luc * f64::from(0.19f32)) as i32,
        luc: i32::from(def.luc),
        wis: i32::from(def.wis),
        attack_var: 0,
        element: def.attrib,
        size: def.size,
        ..Default::default()
    }
}

/// A monster's numbers with its debuffs (`CMob::SetAttack` after `CMob::SetSkillAbility`, `docs/original/skills-buffs.md` section
/// 12): the `add` terms, the current WIS in the magic attack, the current LUC in the critical, and no element defence under
/// `wType` 68. The definition's LUC and WIS stay for the formulas that read the definition.
pub fn monster_with(def: &MobDef, e: &crate::buffs::MobEffects) -> Fighter {
    let mut f = monster(def);
    for (numbers, add) in f.kinds.iter_mut().zip(e.adds) {
        numbers.hit += add.hit;
        numbers.evasion += add.evasion;
        numbers.attack += add.attack;
        numbers.defence += add.defence;
    }
    f.kinds[2].attack += e.wis;
    f.critical = (f64::from(i32::from(def.luc) + e.luc) * f64::from(0.19f32)) as i32;
    if e.no_element {
        f.element = 0;
    }
    f
}

/// The chance in percent that a normal attack hits (`CUser::AttackSuccess`), 20 to 99.
pub fn hit_rate(player: &Fighter, mob: &Fighter, kind: usize) -> i32 {
    let (p, m) = (player.kinds[kind], mob.kinds[kind]);
    let (lu, lm) = (f64::from(player.level), f64::from(mob.level));
    let level = lu / (lu + lm) * 160.0;
    let theirs = f64::from(m.hit) / f64::from(p.evasion + m.hit) * 100.0;
    let ours = f64::from(p.hit) / f64::from(m.evasion + p.hit) * 100.0;
    // NaN (0/0) becomes 0 here and the lowest integer in the original: both end at the minimum.
    ((level - (theirs - ours)) as i32).clamp(20, 99)
}

/// One normal attack's outcome: the `0x0503` result and the HP damage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blow {
    pub result: u8,
    pub damage: u16,
}

/// Rolls a normal attack (`CUser::AttackSuccess`, `CUser::AttackDamage`). `close` is a monster within 3 (ranged attacks lose 30 %).
pub fn strike(player: &Fighter, mob: &Fighter, kind: usize, close: bool, rand: &mut impl FnMut() -> u32) -> Blow {
    if (rand() % 100) as i32 >= hit_rate(player, mob, kind) {
        return Blow { result: RESULT_MISS, damage: 0 };
    }
    let (damage, result) = formula(player, mob, kind, close, 0, element(player.element, mob.element), rand);
    Blow { result, damage: at_least_one_to_ten(damage, rand) as u16 }
}

/// The element factor of an attack element against a defence element (`m_AttackAttr`), 1 outside the table.
fn element(attack: u8, defence: u8) -> f64 {
    match (attack, defence) {
        (a, d) if a < 9 && d < 9 => f64::from(ELEMENTS[usize::from(a)][usize::from(d)]),
        _ => 1.0,
    }
}

/// Damage of 10 or less becomes 1 to 10 at random (`CUser::AttackDamage`).
fn at_least_one_to_ten(damage: i32, rand: &mut impl FnMut() -> u32) -> i32 {
    if damage <= 10 { (rand() % 10 + 1) as i32 } else { damage }
}

/// `CUser::AttackDamage`'s formula (`docs/original/combat.md` section 3 step 3) with `bonus` added to the attack: the damage, at most
/// 30000, and the result (hit or critical).
fn formula(
    player: &Fighter,
    mob: &Fighter,
    kind: usize,
    close: bool,
    bonus: i32,
    element: f64,
    rand: &mut impl FnMut() -> u32,
) -> (i32, u8) {
    let (lu, lm) = (f64::from(player.level), f64::from(mob.level));
    let level = f64::from((((lm - lu - 1.0) * 100.0 + lm) / 100.0 * 10.0) as f32).clamp(1.0, 95.0);
    let mut attack = (player.kinds[kind].attack + bonus) as f32;
    if player.attack_var > 0 {
        attack += (rand() % player.attack_var as u32) as f32;
    }
    let short = kind == RANGED && close;
    if short {
        attack = (f64::from(attack) * f64::from(0.7f32)) as f32;
    }
    let attack = f64::from(attack);
    let luck = |rand: &mut dyn FnMut() -> u32| if player.luc > 0 { f64::from(rand() % player.luc as u32 + 1) } else { 0.0 };
    let r = luck(rand);
    let defence = f64::from(mob.kinds[kind].defence);
    let damage = (attack - attack / 100.0 * level) * element - (defence + defence / 100.0 * level) + r * f64::from(0.1f32);
    let mut damage = (damage as f32).max(0.0);
    let critical = ((f64::from(player.critical) / 100.0 * 100.0 - f64::from(mob.luc) * f64::from(0.034f32)) as i32).clamp(5, 99);
    let mut result = RESULT_HIT;
    if ((rand() % 100) as i32) < critical {
        let r2 = f64::from((luck(rand) * f64::from(1.4f32)) as f32);
        damage = (f64::from(damage) * 1.5 + r2 - r2 / 100.0 * level) as f32;
        if short {
            damage = (f64::from(damage) * f64::from(0.7f32)) as f32;
        }
        result = RESULT_CRITICAL;
    }
    ((damage as i32).min(MAX_DAMAGE), result)
}

/// Whether a skill hits a monster (`CUser::AttackSuccess` with a skill, `docs/original/skills-use.md` section 11; the monster going
/// home is the caller's). `mob` is `monster(def)`.
pub fn skill_hits(player: &Fighter, mob: &Fighter, def: &MobDef, skill: &SkillDef, rand: &mut impl FnMut() -> u32) -> bool {
    let resists_state = skill.state_type != 0 && def.resist_states.get(usize::from(skill.state_type) - 1).copied().unwrap_or(false);
    if resists_state || def.resist_skills.contains(&skill.id) {
        return false;
    }
    // Types 2 and 3 add 100 when the monster's LUC (WIS) is 0, in integer division; no row uses them.
    let against = |ours: i32, theirs: i32| if ours + theirs != 0 { 100 * (ours / (theirs + ours)) } else { 0 };
    let kind = usize::from(skill.attack_kind).min(MAGIC);
    match skill.success_type {
        1 => ((rand() % 100) as i32) < i32::from(skill.success_value),
        2 => ((rand() % 100) as i32) < i32::from(skill.success_value) + against(player.luc, mob.luc),
        3 => ((rand() % 100) as i32) < i32::from(skill.success_value) + against(player.wis, mob.wis),
        _ if kind == MAGIC => ((rand() % 100) as i32) >= magic_resist_rate(player, mob),
        _ => ((rand() % 100) as i32) < hit_rate(player, mob, kind),
    }
}

/// The chance in percent that a monster resists a magic skill, 1 to 90 (section 11 step 5).
fn magic_resist_rate(player: &Fighter, mob: &Fighter) -> i32 {
    let (lu, lm) = (f64::from(player.level), f64::from(mob.level));
    let (wu, wm) = (f64::from(player.wis), f64::from(mob.wis));
    let a = (((lm - lu - 2.0) * 100.0 + lm) / (lu + lm) * 1.1 + f64::from(mob.kinds[MAGIC].evasion)).clamp(1.0, 70.0);
    let b = ((wm * 11.0 - wu * 10.0) / (wu + wm) * 3.9 + f64::from(player.kinds[MAGIC].hit)).clamp(1.0, 70.0);
    // NaN (0/0) becomes 0 here and the lowest integer in the original: both end at the minimum.
    ((a + b) as i32).clamp(1, 90)
}

/// A skill's outcome on a monster: the result (hit or critical) and the HP, SP and MP damage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillBlow {
    pub result: u8,
    pub damage: [u16; 3],
}

/// A skill's damage (`CUser::AttackDamage` with a skill, section 12). `caster` and `mob_health` are current HP, SP, MP; `close` a
/// monster within 3 (ranged skills lose 30 % unless a passive skill keeps them).
#[allow(clippy::too_many_arguments)]
pub fn skill_damage(
    player: &Fighter,
    mob: &Fighter,
    def: &MobDef,
    mob_health: [u32; 3],
    caster: [u16; 3],
    skill: &SkillDef,
    close: bool,
    rand: &mut impl FnMut() -> u32,
) -> SkillBlow {
    let w = skill.damage.map(i64::from);
    let capped = |v: i64| v.clamp(0, i64::from(MAX_DAMAGE)) as u16;
    let each = |base: [i64; 3], f: &dyn Fn(i64, i64) -> i64| -> [u16; 3] {
        std::array::from_fn(|i| if w[i] != 0 { capped(f(w[i], base[i]).max(1)) } else { 0 })
    };
    let percent = |w: i64, of: i64| (w as f64 / 100.0 * of as f64) as i64;
    let hit = |damage| SkillBlow { result: RESULT_HIT, damage };
    match skill.damage_type {
        0 => return hit(skill.damage),
        3 => return hit(each(caster.map(i64::from), &|w, of| w * of)),
        4 => return hit(each([i64::from(def.hp), i64::from(def.sp), i64::from(def.mp)], &percent)),
        5 => return hit(each(mob_health.map(i64::from), &percent)),
        _ => {}
    }
    let kind = usize::from(skill.attack_kind).min(MAGIC);
    let attack_element = if skill.element != 0 { skill.element } else { player.element };
    let factor = element(attack_element, if skill.kind == skills::NEUTRAL_ELEMENT { 0 } else { mob.element });
    let (damage, result) = if skill.damage_type == 2 {
        let (d, result) = formula(player, mob, kind, close, 0, factor, rand);
        let d = [i64::from(d), 0, 0];
        (std::array::from_fn(|i| if w[i] != 0 { d[i] * w[i] } else { d[i] }), result)
    } else {
        let (d, result) = formula(player, mob, kind, close, skill.damage[0] as i32, factor, rand);
        ([i64::from(d), w[1], w[2]], result)
    };
    let mut damage = damage.map(capped);
    damage[0] = at_least_one_to_ten(i32::from(damage[0]), rand) as u16;
    SkillBlow { result, damage }
}

/// The chance in percent that a monster's melee or ranged attack hits a player (`CMob::AttackSuccess`, kinds 0 and 1; normal
/// attacks are always melee), 5 to 95.
pub fn mob_hit_rate(mob: &Fighter, player: &Fighter, kind: usize) -> i32 {
    let (m, p) = (mob.kinds[kind], player.kinds[kind]);
    let (lu, lm) = (f64::from(player.level), f64::from(mob.level));
    let level = lm / (lu + lm) * 160.0;
    let ours = f64::from(p.hit) / f64::from(m.evasion + p.hit) * 100.0;
    let theirs = f64::from(m.hit) / f64::from(p.evasion + m.hit) * 100.0;
    // NaN (0/0) becomes 0 here and the lowest integer in the original: both end at the minimum.
    ((level - (ours - theirs)) as i32).clamp(5, 95)
}

/// Rolls a monster's normal attack on a player (`CMob::AttackSuccess`, `CMob::AttackDamage`; `docs/original/mob-fight.md`
/// section 6). `mob` is `monster(def)`, `damage` and `damage_add` the entry's, `strength` what a script made of it (`m_fStr`, 1.0
/// otherwise), `player` the player's numbers and `defence_element` its upper armour's element.
#[allow(clippy::too_many_arguments)]
pub fn mob_strike(
    mob: &Fighter,
    damage: u16,
    damage_add: u16,
    strength: f32,
    player: &Fighter,
    defence_element: u8,
    rand: &mut impl FnMut() -> u32,
) -> Blow {
    if (rand() % 100) as i32 >= mob_hit_rate(mob, player, MELEE) {
        return Blow { result: RESULT_MISS, damage: 0 };
    }
    let (lu, lm) = (f64::from(player.level), f64::from(mob.level));
    let level = f64::from((((lm - lu - 1.0) * 100.0 + lm) / 100.0 * 10.0) as f32).clamp(1.0, 95.0);
    let mut attack = f32::from(damage) * strength + mob.kinds[MELEE].attack as f32;
    if damage_add > 0 {
        attack += (rand() % u32::from(damage_add)) as f32;
    }
    let attack = f64::from(attack);
    // Only the table's first row: the monster's own element does not count.
    let element = if defence_element < 9 { f64::from(ELEMENTS[0][usize::from(defence_element)]) } else { 1.0 };
    let luck = |rand: &mut dyn FnMut() -> u32| if mob.luc > 0 { f64::from(rand() % mob.luc as u32 + 1) } else { 0.0 };
    let r = luck(rand);
    let defence = f64::from(player.kinds[MELEE].defence);
    let hurt = (attack + attack / 100.0 * level) * element - (defence - defence / 100.0 * level) + r * f64::from(0.1f32);
    let mut hurt = (hurt as f32).max(0.0);
    let critical = ((f64::from(mob.critical) / 100.0 * 100.0 - f64::from(player.luc) * f64::from(0.034f32)) as i32).clamp(5, 99);
    let mut result = RESULT_HIT;
    if ((rand() % 100) as i32) < critical {
        let r2 = f64::from((luck(rand) * f64::from(1.4f32)) as f32);
        hurt = (r2 / 100.0 * level + f64::from(hurt) * 1.5 + r2) as f32;
        result = RESULT_CRITICAL;
    }
    let mut hurt = (hurt as i32).min(MAX_DAMAGE);
    if hurt <= 10 {
        hurt = (rand() % 10 + 1) as i32;
    }
    Blow { result, damage: hurt as u16 }
}

/// The chance in percent that a player's normal blow hits another player (`CUser::AttackSuccess` 0x456b10,
/// `docs/original/pvp.md` section 4 step 2), 1 to 99.
pub fn pvp_hit_rate(attacker: &Fighter, target: &Fighter, kind: usize) -> i32 {
    let (a, t) = (attacker.kinds[kind], target.kinds[kind]);
    let (la, lt) = (f64::from(attacker.level), f64::from(target.level));
    let level = la / (lt + la) * 160.0;
    let theirs = f64::from(t.hit) / f64::from(a.evasion + t.hit) * 100.0;
    let ours = f64::from(a.hit) / f64::from(t.evasion + a.hit) * 100.0;
    // NaN (0/0) becomes 0 here and the lowest integer in the original: both end at the minimum.
    ((level - (theirs - ours)) as i32).clamp(1, 99)
}

/// Rolls a player's normal blow on another player after the block (`CUser::AttackSuccess`, `CUser::AttackDamage` 0x457030;
/// `docs/original/pvp.md` section 4 steps 2 and 5): no level factor, attack less defence, the element against the target's
/// `defence_element`, the critical against the target's LUC. `close`: the two are within 3 (a ranged blow loses 30 % unless
/// `cross_road`, and again after a critical whatever it has).
#[allow(clippy::too_many_arguments)]
pub fn pvp_strike(
    attacker: &Fighter,
    target: &Fighter,
    kind: usize,
    close: bool,
    cross_road: bool,
    defence_element: u8,
    rand: &mut impl FnMut() -> u32,
) -> Blow {
    if (rand() % 100) as i32 >= pvp_hit_rate(attacker, target, kind) {
        return Blow { result: RESULT_MISS, damage: 0 };
    }
    let mut d = attacker.kinds[kind].attack - target.kinds[kind].defence;
    if attacker.attack_var > 0 {
        d += (rand() % attacker.attack_var as u32) as i32;
    }
    let mut d = d.max(0);
    if kind == RANGED && close && !cross_road {
        d = (f64::from(d) * f64::from(0.7f32)) as i32;
    }
    d = (f64::from(d) * element(attacker.element, defence_element)) as i32;
    let luck = |rand: &mut dyn FnMut() -> u32| if attacker.luc > 0 { (rand() % attacker.luc as u32 + 1) as i32 } else { 0 };
    let r = luck(rand);
    d = (f64::from(r) * f64::from(0.1f32) + f64::from(d)) as i32;
    let critical = ((f64::from(attacker.critical) / 100.0 * 100.0 - f64::from(target.luc) * f64::from(0.034f32)) as i32).clamp(1, 99);
    let mut result = RESULT_HIT;
    if ((rand() % 100) as i32) < critical {
        let r2 = luck(rand);
        d = (f64::from(d) * 0.5 + f64::from(r2) * f64::from(1.4f32) + f64::from(d)) as i32;
        if kind == RANGED && close {
            d = (f64::from(d) * f64::from(0.7f32)) as i32;
        }
        result = RESULT_CRITICAL;
    }
    Blow { result, damage: at_least_one_to_ten(d.min(MAX_DAMAGE), rand) as u16 }
}

/// Whether a player's skill hits another player (`CUser::AttackSuccess` with a skill, `docs/original/pvp-2.md` section 5, after the
/// block and the state protections, which are the caller's): `bySuccessType`, else the kind's rate; magic is resisted by the target.
pub fn pvp_skill_hits(caster: &Fighter, target: &Fighter, skill: &SkillDef, rand: &mut impl FnMut() -> u32) -> bool {
    // Types 2 and 3 in integer division, as on monsters; no row uses them.
    let against = |ours: i32, theirs: i32| if ours + theirs != 0 { 100 * (ours / (theirs + ours)) } else { 0 };
    let roll = || (rand() % 100) as i32;
    let mut roll = roll;
    match (skill.success_type, usize::from(skill.attack_kind)) {
        (1, _) => roll() < i32::from(skill.success_value),
        (2, _) => roll() < i32::from(skill.success_value) + against(caster.luc, target.luc),
        (3, _) => roll() < i32::from(skill.success_value) + against(caster.wis, target.wis),
        (_, MAGIC) => roll() >= pvp_resist_rate(caster, target),
        (_, kind @ (MELEE | RANGED)) => roll() < pvp_hit_rate(caster, target, kind),
        // Any other kind always hits.
        _ => true,
    }
}

/// The chance in percent that a player resists another's magic skill, 1 to 90 (section 5 step 4): the target's level and magic
/// evasion, its WIS against the caster's and the caster's magic hit.
fn pvp_resist_rate(caster: &Fighter, target: &Fighter) -> i32 {
    let (la, lt) = (f64::from(caster.level), f64::from(target.level));
    let (wa, wt) = (f64::from(caster.wis), f64::from(target.wis));
    let a = f64::from(((((lt - la - 2.0) * 100.0 + lt) / (la + lt)) as f32 * 1.1 + target.kinds[MAGIC].evasion as f32).clamp(1.0, 70.0));
    let b = f64::from((((wt * 11.0 - wa * 10.0) / (wa + wt)) as f32 * 3.9 + caster.kinds[MAGIC].hit as f32).clamp(1.0, 70.0));
    // NaN (0/0) becomes 0 here and the lowest integer in the original: both end at the minimum.
    ((a + b) as i32).clamp(1, 90)
}

/// What a skill's damage on a player reads besides the two players' numbers: the target's defence element, both players' HP, SP, MP
/// and the target's maxima (in that order), and whether the two are within 3 (ranged skills lose 30 % unless the caster has the cross
/// road passive, and again after a critical whatever it has).
pub struct PvpSkillHit<'a> {
    pub caster: &'a Fighter,
    pub target: &'a Fighter,
    pub defence_element: u8,
    pub caster_health: [u16; 3],
    pub target_health: [u16; 3],
    pub target_max: [u16; 3],
    pub within_three: bool,
    pub cross_road: bool,
}

/// A skill's damage on a player (`CUser::AttackDamage(CUser*, kind, damage, skill)` 0x457030, `docs/original/pvp-2.md` section 6).
pub fn pvp_skill_damage(h: &PvpSkillHit, skill: &SkillDef, rand: &mut impl FnMut() -> u32) -> SkillBlow {
    let w = skill.damage.map(i64::from);
    let capped = |v: i64| v.clamp(0, i64::from(MAX_DAMAGE)) as u16;
    let each = |base: [i64; 3], f: &dyn Fn(i64, i64) -> i64| -> [u16; 3] {
        std::array::from_fn(|i| if w[i] != 0 { capped(f(w[i], base[i]).max(1)) } else { 0 })
    };
    let percent = |w: i64, of: i64| (w as f32 / 100.0 * of as f32) as i64;
    let hit = |damage| SkillBlow { result: RESULT_HIT, damage };
    match skill.damage_type {
        0 => return hit(skill.damage),
        3 => return hit(each(h.caster_health.map(i64::from), &|w, of| w * of)),
        4 => return hit(each(h.target_max.map(i64::from), &percent)),
        5 => return hit(each(h.target_health.map(i64::from), &percent)),
        _ => {}
    }
    let (a, t) = (h.caster, h.target);
    let attack_element = if skill.element != 0 { skill.element } else { a.element };
    let factor = element(attack_element, if skill.kind == skills::NEUTRAL_ELEMENT { 0 } else { h.defence_element });
    let bonus = if skill.damage_type == 1 { i32::from(skill.damage[0]) } else { 0 };
    let luck = |rand: &mut dyn FnMut() -> u32| if a.luc > 0 { f64::from(rand() % a.luc as u32 + 1) } else { 0.0 };
    let critical = ((f64::from(a.critical) / 100.0 * 100.0 - f64::from(t.luc) * f64::from(0.034f32)) as i32).clamp(1, 99);
    let mut result = RESULT_HIT;
    let first = match usize::from(skill.attack_kind) {
        kind @ (MELEE | RANGED) => {
            let mut d = bonus + a.kinds[kind].attack - t.kinds[kind].defence;
            if a.attack_var > 0 {
                d += (rand() % a.attack_var as u32) as i32;
            }
            let mut d = d.max(0);
            if kind == RANGED && h.within_three && !h.cross_road {
                d = (f64::from(d) * f64::from(0.7f32)) as i32;
            }
            d = (f64::from(d) * factor) as i32;
            d = (luck(rand) * f64::from(0.1f32) + f64::from(d)) as i32;
            if ((rand() % 100) as i32) < critical {
                d = (f64::from(d) * 0.5 + luck(rand) * f64::from(1.4f32) + f64::from(d)) as i32;
                // Within 3 the cut comes again, without the cross road test.
                if kind == RANGED && h.within_three {
                    d = (f64::from(d) * f64::from(0.7f32)) as i32;
                }
                result = RESULT_CRITICAL;
            }
            d
        }
        MAGIC => {
            let (la, lt) = (f64::from(a.level), f64::from(t.level));
            let p1 = f64::from(((101.0 * lt - 100.0 - 100.0 * la) / (la + lt)) as f32 * 2.4).clamp(1.0, 50.0);
            let p2 = f64::from((t.kinds[MAGIC].defence as f32 / (a.item_attack + a.int) as f32 * 100.0).clamp(1.0, 70.0));
            let x = (factor as f32 * (a.kinds[MAGIC].attack + bonus) as f32) as f64;
            let mut d = f64::from(((x - x / 100.0 * (p1 + p2)) as f32).max(0.0));
            d = f64::from((luck(rand) * f64::from(0.1f32) + d) as f32);
            if ((rand() % 100) as i32) < critical {
                d = f64::from((d * 0.5 + luck(rand) * f64::from(1.4f32) + d) as f32);
                result = RESULT_CRITICAL;
            }
            d as i32
        }
        // Another kind: no formula, only the skill's own damage.
        _ => bonus,
    };
    let first = i64::from(first);
    let damage = match skill.damage_type {
        1 => [first, w[1], w[2]],
        2 if w[0] != 0 => [first * w[0], 0, 0],
        _ => [first, 0, 0],
    };
    let mut damage = damage.map(capped);
    damage[0] = at_least_one_to_ten(i32::from(damage[0]), rand) as u16;
    SkillBlow { result, damage }
}

/// Whether a monster's skill hits a player (`CMob::AttackSuccess` with a skill, `docs/original/mob-skills.md` section 7, after the
/// player's block, which is the caller's). `mob` is `monster_with`, `def` its definition. Lapis state resistance and the item
/// protections are the caller's too (`zone::mob_skills`).
pub fn mob_skill_hits(mob: &Fighter, def: &MobDef, player: &Fighter, skill: &SkillDef, rand: &mut impl FnMut() -> u32) -> bool {
    // Types 2 and 3 compare the monster's current value with its definition's, in integer division; no monster skill uses them.
    let against = |current: i32, base: i32| if current + base != 0 { 100 * (current / (base + current)) } else { 0 };
    let kind = usize::from(skill.attack_kind).min(MAGIC);
    let roll = (rand() % 100) as i32;
    match skill.success_type {
        1 => roll < i32::from(skill.success_value),
        2 => roll < i32::from(skill.success_value) + against(mob.luc, i32::from(def.luc)),
        3 => roll < i32::from(skill.success_value) + against(mob.wis, i32::from(def.wis)),
        _ if kind == MAGIC => roll >= player_resist_rate(mob, def, player),
        _ => roll < mob_hit_rate(mob, player, kind),
    }
}

/// The chance in percent that a player resists a monster's magic skill, 1 to 90 (section 7): the player's level and magic evasion
/// against the monster's, its WIS against the monster definition's.
fn player_resist_rate(mob: &Fighter, def: &MobDef, player: &Fighter) -> i32 {
    let (lu, lm) = (f64::from(player.level), f64::from(mob.level));
    let (wu, wm) = (f64::from(player.wis), f64::from(def.wis));
    let a = (((lu - lm - 2.0) * 100.0 + lu) / (lu + lm) * 1.1 + f64::from(player.kinds[MAGIC].evasion)).clamp(1.0, 70.0);
    let b = ((wu * 11.0 - wm * 10.0) / (wu + wm) * 3.9 + f64::from(mob.kinds[MAGIC].hit)).clamp(1.0, 70.0);
    // NaN (0/0) becomes 0 here and the lowest integer in the original: both end at the minimum.
    ((a + b) as i32).clamp(1, 90)
}

/// What a monster skill's damage reads besides the skill: the monster's numbers (`monster_with`) and entry 0's damage and random
/// extra, the monster's HP, SP, MP; the player's numbers, its defence element, its HP, SP, MP and their maxima.
pub struct MobSkillHit<'a> {
    pub mob: &'a Fighter,
    pub entry_damage: u16,
    pub entry_damage_add: u16,
    pub mob_health: [u32; 3],
    pub player: &'a Fighter,
    pub defence_element: u8,
    pub health: [u16; 3],
    pub max: [u16; 3],
}

/// A monster skill's damage on a player (`CMob::AttackDamage` with a skill, `docs/original/mob-skills.md` section 7).
pub fn mob_skill_damage(h: &MobSkillHit, skill: &SkillDef, rand: &mut impl FnMut() -> u32) -> SkillBlow {
    let w = skill.damage.map(i64::from);
    let capped = |v: i64| v.clamp(0, i64::from(MAX_DAMAGE)) as u16;
    let each = |base: [i64; 3], f: &dyn Fn(i64, i64) -> i64| -> [u16; 3] {
        std::array::from_fn(|i| if w[i] != 0 { capped(f(w[i], base[i]).max(1)) } else { 0 })
    };
    let percent = |w: i64, of: i64| (w as f32 / 100.0 * of as f32) as i64;
    let hit = |damage| SkillBlow { result: RESULT_HIT, damage };
    match skill.damage_type {
        0 => return hit(skill.damage),
        3 => return hit(each(h.mob_health.map(i64::from), &|w, of| w * of)),
        4 => return hit(each(h.max.map(i64::from), &percent)),
        5 => return hit(each(h.health.map(i64::from), &percent)),
        _ => {}
    }
    let kind = usize::from(skill.attack_kind).min(MAGIC);
    let (m, p) = (h.mob, h.player);
    let (lu, lm) = (f64::from(p.level), f64::from(m.level));
    let level = f64::from((((lm - lu - 1.0) * 100.0 + lm) / 100.0 * 10.0) as f32).clamp(1.0, 95.0);
    let bonus = if skill.damage_type == 1 { i32::from(skill.damage[0]) } else { 0 };
    let mut attack = (i32::from(h.entry_damage) + m.kinds[kind].attack + bonus) as f32;
    if h.entry_damage_add > 0 {
        attack += (rand() % u32::from(h.entry_damage_add)) as f32;
    }
    let attack = f64::from(attack);
    let factor = if skill.kind == skills::NEUTRAL_ELEMENT { element(skill.element, 0) } else { element(skill.element, h.defence_element) };
    let luck = |rand: &mut dyn FnMut() -> u32| if m.luc > 0 { f64::from(rand() % m.luc as u32 + 1) } else { 0.0 };
    let r = luck(rand);
    let defence = f64::from(p.kinds[kind].defence);
    let hurt = (attack + attack / 100.0 * level) * factor - (defence - defence / 100.0 * level) + r * f64::from(0.1f32);
    let mut hurt = (hurt as f32).max(0.0);
    let critical = ((f64::from(m.critical) / 100.0 * 100.0 - f64::from(p.luc) * f64::from(0.034f32)) as i32).clamp(5, 99);
    let mut result = RESULT_HIT;
    if ((rand() % 100) as i32) < critical {
        // No `+ R2` here, unlike the normal attack.
        let r2 = f64::from((luck(rand) * f64::from(1.4f32) / 100.0 * level) as f32);
        hurt = (f64::from(hurt) * 1.5 + r2) as f32;
        result = RESULT_CRITICAL;
    }
    let first = i64::from(hurt as i32);
    let damage = if skill.damage_type == 1 {
        [first, w[1], w[2]]
    } else if skill.damage_type == 2 && w[0] != 0 {
        [first * w[0], 0, 0]
    } else {
        [first, 0, 0]
    };
    let mut damage = damage.map(capped);
    damage[0] = at_least_one_to_ten(i32::from(damage[0]), rand) as u16;
    SkillBlow { result, damage }
}

/// What a monster gives a player of `level` (`CMob::GetExp`), before the killer's 1.2. `rate` is the experience event's factor
/// (`g_fExp2XValue`; 1.0 when there is none): applied to the truncated base and truncated again
/// (`docs/original/operator-console.md` section 2).
pub fn mob_exp(def: &MobDef, level: u16, rate: f64) -> u32 {
    let d = i32::from(def.level) - i32::from(level);
    if d > 9 {
        return 0;
    }
    let exp = (f64::from(def.exp) * (f64::from(d + 1) * 0.2 + 1.0)) as i64;
    let exp = if rate == 1.0 { exp } else { (exp as f64 * rate) as i64 };
    exp.max(0) as u32
}

/// The killer's share (`CMob::Death`).
pub fn kill_exp(def: &MobDef, level: u16, rate: f64) -> u32 {
    (f64::from(mob_exp(def, level, rate)) * 1.2) as u32
}

/// What changes with experience: level, experience, points and base stats (STR, DEX, REC, INT, WIS, LUC).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub level: u16,
    pub exp: u32,
    pub stat_point: u16,
    pub skill_point: u16,
    pub stats: [u16; 6],
}

/// The highest level of a mode (`CUser::AddExpFromUser`): 30 in easy mode, 60 otherwise.
pub fn max_level(grow: u8) -> u16 {
    if grow == 0 { 30 } else { 60 }
}

/// The stat each level adds to (fighter STR, defender REC, ranger DEX, archer LUC, mage INT, priest WIS), as an index of
/// STR, DEX, REC, INT, WIS, LUC.
pub fn class_stat(job: u8) -> Option<usize> {
    [STR, REC, DEX, LUC, INT, WIS].get(usize::from(job)).copied()
}

/// Adds experience (`CUser::AddExpFromUser`, `CUser::LevelUpStatus`). `None` when nothing changes (no gain, or at the maximum
/// level); otherwise the new progress and how many levels it gained.
pub fn add_exp(progress: Progress, job: u8, grow: u8, gain: u32, table: &ExpTable) -> Option<(Progress, u16)> {
    let max = max_level(grow);
    if gain == 0 || progress.level >= max {
        return None;
    }
    let mut p = progress;
    let exp = p.exp.saturating_add(gain);
    let mut levels = 0;
    while exp >= table.total(grow, p.level) {
        p.level += 1;
        levels += 1;
        if let Some(stat) = class_stat(job) {
            p.stats[stat] = p.stats[stat].saturating_add(1);
        }
        let (stat_points, skill_points) = match grow {
            2 => (7, 4),
            3 => (9, 5),
            _ => (5, 3),
        };
        p.stat_point = p.stat_point.saturating_add(stat_points);
        p.skill_point = p.skill_point.saturating_add(skill_points);
        if p.level >= max {
            p.exp = table.total(grow, p.level - 1);
            return Some((p, levels));
        }
    }
    p.exp = exp;
    Some((p, levels))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemDef;

    fn mob(level: u16) -> MobDef {
        MobDef { id: 1, level, hp: 100, dex: 10, luc: 10, defense: 10, exp: 20, ..Default::default() }
    }

    #[test]
    fn attack_waits_and_range() {
        assert_eq!(attack_interval(0), Duration::from_millis(4000));
        assert_eq!(attack_interval(9), Duration::from_millis(1000));
        assert_eq!(attack_interval(200), Duration::from_millis(1000));
        assert_eq!(range_slack(2, None), 15.0);
        assert_eq!(range_slack(2, Some(Duration::from_millis(500))), 7.5);
        assert_eq!(range_slack(2, Some(Duration::from_millis(50))), 2.0, "at least 2");
        assert_eq!(range_slack(2, Some(Duration::from_millis(1001))), 15.0);
        assert!(in_range(20.3, 15.0, 2, 1));
        assert!(!in_range(20.4, 15.0, 2, 1));
    }

    #[test]
    fn numbers_follow_set_attack() {
        // STR 20, DEX 10, REC 8, INT 4, WIS 6, LUC 16 at level 10, weapon attack 30, armour defence 5.
        let gear = Gear { attack: 30, defence: 5, magic_defence: 3, kind: Some(MELEE), ..Default::default() };
        let p = player(10, [20, 10, 8, 4, 6, 16], &gear);
        // 20 * 1.3f (1.29999995) + 30 + 2 = 57.99999...: 57, as in the original.
        assert_eq!(p.kinds[MELEE], Numbers { hit: 15, evasion: 17, attack: 57, defence: 13 });
        // 16 * 0.3 + 30 + 20 + 2 = 56.8.
        assert_eq!(p.kinds[RANGED].attack, 56);
        assert_eq!(p.kinds[2], Numbers { hit: 0, evasion: 0, attack: 36, defence: 9 });
        assert_eq!(p.critical, 3, "16 * 0.19 = 3.04");
        let m = monster(&MobDef { defense_add: 4, wis: 7, magic: 2, ..mob(8) });
        assert_eq!(m.kinds[MELEE], Numbers { hit: 13, evasion: 10, attack: 0, defence: 10 }, "5 + 4 + 1.25");
        assert_eq!(m.kinds[RANGED], Numbers { hit: 13, evasion: 14, attack: 0, defence: 14 });
        assert_eq!(m.kinds[2], Numbers { hit: 0, evasion: 0, attack: 15, defence: 2 });
    }

    #[test]
    fn worn_equipment_gives_less() {
        // Weapon of quality 100 with effects 40 and 10; armour with defence 9.
        let defs = ItemDefs::new([
            ItemDef { item_type: 13, type_id: 1, quality: 100, effects: [40, 10, 0, 0], range: 20, attrib: 3, ..Default::default() },
            ItemDef { item_type: 16, type_id: 1, quality: 100, effects: [0, 0, 9, 4], ..Default::default() },
        ]);
        let unit = |slot, item_type, quality| ItemUnit { bag: 0, slot, item_type, type_id: 1, count: 1, quality, ..Default::default() };
        let g = gear(&[unit(5, 13, 51), unit(0, 16, 50)], &defs, &Passives::default());
        assert_eq!(
            g,
            Gear {
                attack: 40,
                attack_var: 10,
                defence: 4,
                magic_defence: 2,
                range: 20,
                element: 3,
                kind: Some(RANGED),
                armour_element: 0,
                ..Default::default()
            }
        );
        let g = gear(&[unit(5, 13, 10)], &defs, &Passives::default());
        assert_eq!((g.attack, g.attack_var), (0, 0), "quality at a tenth: nothing");
        assert_eq!(gear(&[], &defs, &Passives::default()).kind, None);
    }

    #[test]
    fn lapis_and_enchant_steps_add_to_the_gear() {
        // A craftable bow with a fire lapis (attack 5, range 2, element 1) and a water one (element 2), at enchant step 3; armour
        // with a lapis of absorption 4 blocking state 7, at armour step 2 (52); the cloak's badge counts for nothing.
        let defs = ItemDefs::new([
            ItemDef { item_type: 13, type_id: 1, quality: 100, effects: [40, 0, 0, 0], attrib: 0, craftable: true, ..Default::default() },
            ItemDef { item_type: 17, type_id: 1, quality: 100, effects: [0, 0, 9, 0], craftable: true, ..Default::default() },
            ItemDef { item_type: 24, type_id: 1, ..Default::default() },
            ItemDef { item_type: 30, type_id: 1, effects: [5, 0, 0, 0], range: 2, attrib: 1, ..Default::default() },
            ItemDef { item_type: 30, type_id: 2, attrib: 2, ..Default::default() },
            ItemDef { item_type: 30, type_id: 3, absorption: 4, special: 7, effects: [0, 0, 3, 0], ..Default::default() },
        ]);
        let unit = |slot, item_type, gems, craft: &[u8]| ItemUnit {
            bag: 0,
            slot,
            item_type,
            type_id: 1,
            count: 1,
            quality: 100,
            gems,
            craft_name: craft.to_vec(),
        };
        let bow = unit(5, 13, [1, 0, 0, 0, 0, 0], b"00000000000000000003");
        let armour = unit(1, 17, [3, 0, 0, 0, 0, 0], b"00000000000000000052");
        let cloak = unit(7, 24, [1, 1, 3, 0, 0, 0], b"");
        let g = gear(&[bow.clone(), armour, cloak], &defs, &Passives::default());
        assert_eq!((g.attack, g.range, g.element), (40 + 5 + 21, 2, 1));
        assert_eq!((g.defence, g.absorption, g.state_blocks), (12, 4 + 10, 1 << 7));
        let mixed = unit(5, 13, [1, 2, 0, 0, 0, 0], b"");
        assert_eq!(gear(&[mixed], &defs, &Passives::default()).element, 0, "two elements cancel");
        assert_eq!(absorb(30, 14), (16, 14));
        assert_eq!(absorb(10, 14), (1, 9), "at least 1 left");
        assert_eq!(absorb(0, 14), (0, 0));
    }

    #[test]
    fn passive_skills_add_to_the_gear_and_numbers() {
        let defs = ItemDefs::new([
            ItemDef { item_type: 1, type_id: 1, quality: 100, effects: [40, 0, 0, 0], ..Default::default() },
            ItemDef { item_type: 19, type_id: 1, quality: 100, effects: [0, 0, 6, 0], ..Default::default() },
        ]);
        let unit = |slot, item_type| ItemUnit { bag: 0, slot, item_type, type_id: 1, count: 1, quality: 100, ..Default::default() };
        let mut passives = Passives::default();
        passives.weapon_damage[1] = 12;
        passives.weapon_damage[2] = 50;
        passives.shield = 7;
        passives.bonuses.add(15, 2, 0);
        passives.bonuses.add(18, 4, 0);
        passives.bonuses.add(20, 3, 0);
        passives.bonuses.add(23, 30, 0);
        passives.bonuses.add(26, 5, 0);
        passives.bonuses.add(29, 6, 0);
        passives.bonuses.add(25, 9, 0);
        let g = gear(&[unit(5, 1), unit(6, 19)], &defs, &passives);
        assert_eq!((g.attack, g.defence, g.range), (40 + 12, 6 + 7, 2), "the mastery of the worn weapon's real type, the shield value");
        assert_eq!(gear(&[unit(5, 1)], &defs, &passives).defence, 0, "no shield, no shield value");
        // As numbers_follow_set_attack, with weapon attack 52 and the adds.
        let p = player(10, [20, 10, 8, 4, 6, 16], &Gear { defence: 5, magic_defence: 3, kind: Some(MELEE), ..g });
        assert_eq!(p.kinds[MELEE], Numbers { hit: 18, evasion: 23, attack: 26 + 30 + 52 + 2 - 1, defence: 18 });
        assert_eq!(p.kinds[RANGED], Numbers { hit: 15, evasion: 17, attack: 78, defence: 13 }, "melee adds stay melee");
        assert_eq!(p.kinds[2].attack, 36 + 22 + 9);
        assert_eq!(p.critical, 3 + 4);
    }

    #[test]
    fn hits_follow_levels_and_numbers() {
        let p = Fighter { level: 10, kinds: [Numbers { hit: 15, evasion: 17, attack: 58, defence: 13 }; 3], ..Default::default() };
        let m = monster(&mob(10));
        // Monster evasion 5 + 5 + 1.25 = 11: 80 - (15/(17+15) - 15/(11+15)) * 100 = 80 - 46.875 + 57.69.
        assert_eq!(hit_rate(&p, &m, MELEE), 90);
        assert_eq!(hit_rate(&Fighter::default(), &Fighter::default(), MELEE), 20, "0/0");
        let mut rolls = [90].into_iter();
        assert_eq!(strike(&p, &m, MELEE, false, &mut || rolls.next().unwrap()), Blow { result: RESULT_MISS, damage: 0 });
    }

    #[test]
    fn damage_follows_the_formula() {
        let p = Fighter { level: 10, luc: 16, kinds: [Numbers { hit: 99, evasion: 0, attack: 100, defence: 0 }; 3], ..Default::default() };
        let m = Fighter { level: 10, kinds: [Numbers { defence: 20, ..Default::default() }; 3], ..Default::default() };
        // P = (-100 + 10) / 100 * 10 = -9, so 1. (100 - 1) * 1 - (20 + 0.2) + 5 * 0.1 = 79.3. No critical (roll 50 >= 5).
        let mut rolls = [0, 4, 50].into_iter();
        assert_eq!(strike(&p, &m, MELEE, false, &mut || rolls.next().unwrap()), Blow { result: RESULT_HIT, damage: 79 });
        // Critical: 79.3 * 1.5 + 2 * 1.4 - 2.8 / 100 = 121.722.
        let mut rolls = [0, 4, 0, 1].into_iter();
        assert_eq!(strike(&p, &m, MELEE, false, &mut || rolls.next().unwrap()), Blow { result: RESULT_CRITICAL, damage: 121 });
        // Ranged and close: 70 attack. Element 1 against 2: 0.5.
        let p = Fighter { element: 1, ..p };
        let m = Fighter { element: 2, ..m };
        let mut rolls = [0, 4, 50].into_iter();
        // (70 - 0.7) * 0.5 - 20.2 + 0.5 = 14.95.
        assert_eq!(strike(&p, &m, RANGED, true, &mut || rolls.next().unwrap()).damage, 14);
        // Up to 10 becomes 1 to 10.
        let weak = Fighter { kinds: [Numbers { hit: 99, attack: 1, ..Default::default() }; 3], ..p };
        let mut rolls = [0, 50, 6, 6].into_iter();
        assert_eq!(strike(&weak, &m, MELEE, false, &mut || rolls.next().unwrap()).damage, 7);
    }

    #[test]
    fn skills_hit_by_their_own_rules() {
        let p = Fighter { level: 10, wis: 10, kinds: [Numbers { hit: 15, evasion: 17, attack: 58, defence: 13 }; 3], ..Default::default() };
        let mut def = MobDef { level: 20, wis: 30, ..mob(20) };
        let m = monster(&def);
        let skill = SkillDef { id: 9, attack_kind: 2, ..Default::default() };
        let roll = |skill: &SkillDef, def: &MobDef, r: u32| skill_hits(&p, &monster(def), def, skill, &mut || r);
        // Magic: a = (820 / 30) * 1.1 = 30.07, b = (330 - 100) / 40 * 3.9 + 15 (magic hit) = 37.43: resisted below 67.
        assert_eq!(magic_resist_rate(&p, &m), 67);
        assert!(!roll(&skill, &def, 66));
        assert!(roll(&skill, &def, 67));
        // Type 1: the value alone.
        let sure = SkillDef { success_type: 1, success_value: 30, ..skill };
        assert!(roll(&sure, &def, 29));
        assert!(!roll(&sure, &def, 30));
        // Melee: the normal attack's rate.
        let melee = SkillDef { attack_kind: 0, ..skill };
        let rate = hit_rate(&p, &m, MELEE) as u32;
        assert!(roll(&melee, &def, rate - 1));
        assert!(!roll(&melee, &def, rate));
        // Resisted states and skills always miss.
        def.resist_states[2] = true;
        assert!(!roll(&SkillDef { state_type: 3, ..sure }, &def, 0));
        assert!(roll(&SkillDef { state_type: 4, ..sure }, &def, 0));
        def.resist_skills[5] = 9;
        assert!(!roll(&sure, &def, 0));
    }

    #[test]
    fn skill_damage_by_damage_type() {
        // As damage_follows_the_formula: attack 100, defence 20, P = 1; luck roll 4 (R = 5), no critical (50).
        let p = Fighter { level: 10, luc: 16, kinds: [Numbers { hit: 99, evasion: 0, attack: 100, defence: 0 }; 3], ..Default::default() };
        let def = MobDef { hp: 1000, sp: 40, ..mob(10) };
        let m = Fighter { level: 10, kinds: [Numbers { defence: 20, ..Default::default() }; 3], ..Default::default() };
        let blow_on = |m: &Fighter, skill: SkillDef, rolls: &[u32]| {
            let mut rolls = rolls.iter().copied();
            skill_damage(&p, m, &def, [300, 40, 0], [100, 50, 30], &skill, false, &mut || rolls.next().unwrap())
        };
        let blow = |skill, rolls: &[u32]| blow_on(&m, skill, rolls);
        let m = Fighter { element: 2, ..m };
        let hit = |damage| SkillBlow { result: RESULT_HIT, damage };
        let skill = |damage_type, damage| SkillDef { damage_type, damage, ..Default::default() };
        assert_eq!(blow(skill(0, [7, 0, 0]), &[]), hit([7, 0, 0]), "as it is, even below 11");
        // Type 1: A = 150. (150 - 1.5) - 20.2 + 0.5 = 128.8; SP and MP as they are.
        assert_eq!(blow(skill(1, [50, 3, 0]), &[4, 50]), hit([128, 3, 0]));
        // Type 2: the formula (79.3), times 2.
        assert_eq!(blow(skill(2, [2, 0, 0]), &[4, 50]), hit([158, 0, 0]));
        // Type 3: the caster's current HP and MP (HP, SP, MP order) times the values.
        assert_eq!(blow(skill(3, [2, 0, 1]), &[]), hit([200, 0, 30]));
        // Type 4: percent of the definition's maxima, at least 1; type 5 of the current values.
        assert_eq!(blow(skill(4, [10, 1, 0]), &[]), hit([100, 1, 0]), "1 % of 40 SP is 0.4: at least 1");
        assert_eq!(blow(skill(5, [50, 0, 0]), &[]), hit([150, 0, 0]));
        // The skill's element (1 against 2: 0.5): 148.5 * 0.5 - 20.2 + 0.5 = 54.8.
        assert_eq!(blow_on(&m, SkillDef { element: 1, ..skill(1, [50, 0, 0]) }, &[4, 50]).damage[0], 54);
        // wType 24 takes column 0 (1 against 0: 1.2): 178.2 - 20.2 + 0.5 = 158.5.
        let neutral = SkillDef { element: 1, kind: skills::NEUTRAL_ELEMENT, ..skill(1, [50, 0, 0]) };
        assert_eq!(blow_on(&m, neutral, &[4, 50]).damage[0], 158);
        // A critical, as in damage_follows_the_formula: 1.
        assert_eq!(blow(skill(1, [0, 0, 0]), &[4, 0, 1]).result, RESULT_CRITICAL);
    }

    #[test]
    fn monsters_hit_back() {
        // Player level 10: hit 15, evasion 17, defence 13, LUC 16. Monster level 12, DEX 10, LUC 10: hit 17, evasion 12 (5 + 6 + 1.25),
        // critical 1.
        let p = Fighter { level: 10, luc: 16, kinds: [Numbers { hit: 15, evasion: 17, attack: 0, defence: 13 }; 3], ..Default::default() };
        let m = monster(&MobDef { dex: 10, luc: 10, ..mob(12) });
        // 160 * 12 / 22 - (15 / 27 - 17 / 34) * 100 = 87.27 - 5.56 = 81.7.
        assert_eq!(mob_hit_rate(&m, &p, MELEE), 81);
        assert_eq!(mob_hit_rate(&Fighter::default(), &Fighter::default(), MELEE), 5, "0/0");
        let mut rolls = [83].into_iter();
        assert_eq!(mob_strike(&m, 30, 0, 1.0, &p, 0, &mut || rolls.next().unwrap()), Blow { result: RESULT_MISS, damage: 0 });
        // P = (100 + 12) / 100 * 10 = 11.2. A = 30 + 4 (the extra roll). (34 + 3.808) * 1 - (13 - 1.456) + 3 * 0.1 = 26.652.
        let mut rolls = [0, 4, 2, 50].into_iter();
        assert_eq!(mob_strike(&m, 30, 10, 1.0, &p, 0, &mut || rolls.next().unwrap()), Blow { result: RESULT_HIT, damage: 26 });
        // Upper armour of element 5: 0.7. (37.808 * 0.7) - 11.544 + 0.3 = 15.2216.
        let mut rolls = [0, 4, 2, 50].into_iter();
        assert_eq!(mob_strike(&m, 30, 10, 1.0, &p, 5, &mut || rolls.next().unwrap()).damage, 15);
        // Critical (chance 5): R2 = 3 * 1.4 = 4.2; 26.652 * 1.5 + 4.2 * 0.112 + 4.2 = 44.6484.
        let mut rolls = [0, 4, 2, 4, 2].into_iter();
        assert_eq!(mob_strike(&m, 30, 10, 1.0, &p, 0, &mut || rolls.next().unwrap()), Blow { result: RESULT_CRITICAL, damage: 44 });
    }

    #[test]
    fn experience_by_level_difference() {
        let def = mob(10);
        assert_eq!(mob_exp(&def, 10, 1.0), 24, "20 * 1.2");
        assert_eq!(mob_exp(&def, 1, 1.0), 60, "9 below: 20 * (10 * 0.2 + 1)");
        assert_eq!(mob_exp(&mob(20), 10, 1.0), 0, "more than 9 below the monster");
        assert_eq!(mob_exp(&def, 15, 1.0), 3, "20 * (-4 * 0.2 + 1) is 3.99999... in double precision, as in the original");
        assert_eq!(mob_exp(&def, 16, 1.0), 0);
        assert_eq!(kill_exp(&def, 10, 1.0), 28, "24 * 1.2 = 28.8");
        assert_eq!(
            (mob_exp(&def, 10, 2.0), mob_exp(&def, 10, 0.0), mob_exp(&def, 10, 1.5)),
            (48, 0, 36),
            "the event's factor on the truncated base"
        );
        assert_eq!(mob_exp(&def, 15, 2.0), 6, "3 * 2: the base is cut before the factor");
        assert_eq!(kill_exp(&def, 10, 2.0), 57, "48 * 1.2 = 57.6");
    }

    #[test]
    fn levels_come_with_points_and_a_class_stat() {
        // Normal mode totals: 100, 300, 800.
        let table = ExpTable::from_rows([(1, 2, 100), (2, 2, 200), (3, 2, 500)]);
        let start = Progress { level: 1, exp: 90, stat_point: 0, skill_point: 0, stats: [10; 6] };
        assert_eq!(add_exp(start, 0, 1, 0, &table), None);
        let (p, levels) = add_exp(start, 0, 1, 5, &table).unwrap();
        assert_eq!((p.level, p.exp, levels), (1, 95, 0));
        // 90 + 250 = 340: past 100 and 300, a ranger (DEX) gains two levels.
        let (p, levels) = add_exp(start, 2, 1, 250, &table).unwrap();
        assert_eq!((p.level, p.exp, levels, p.stat_point, p.skill_point), (3, 340, 2, 10, 6));
        assert_eq!(p.stats, [10, 12, 10, 10, 10, 10]);
        // Easy mode stops at 30.
        assert_eq!(add_exp(Progress { level: 30, ..start }, 0, 0, 1000, &table), None);
    }

    #[test]
    fn monster_skills_roll_and_hurt() {
        use crate::mobs::MobDef;
        let def = MobDef { level: 10, ..Default::default() };
        let mob = monster(&def);
        let player = Fighter { level: 10, ..Default::default() };
        let sure = SkillDef { success_type: 1, success_value: 50, ..Default::default() };
        assert!(mob_skill_hits(&mob, &def, &player, &sure, &mut || 49));
        assert!(!mob_skill_hits(&mob, &def, &player, &sure, &mut || 50));
        // Magic: the player resists at the lowest rate, 1 (a: -10.45 clamped to 1, b: 0/0 at the minimum).
        let magic = SkillDef { attack_kind: MAGIC as u8, ..Default::default() };
        assert!(!mob_skill_hits(&mob, &def, &player, &magic, &mut || 0));
        assert!(mob_skill_hits(&mob, &def, &player, &magic, &mut || 1));

        let mob = Fighter { level: 1, ..Default::default() };
        let player = Fighter { level: 1, ..Default::default() };
        let hit = MobSkillHit {
            mob: &mob,
            entry_damage: 50,
            entry_damage_add: 0,
            mob_health: [500, 0, 0],
            player: &player,
            defence_element: 0,
            health: [80, 40, 20],
            max: [200, 100, 50],
        };
        let skill = |damage_type, damage| SkillDef { damage_type, damage, ..Default::default() };
        assert_eq!(mob_skill_damage(&hit, &skill(0, [7, 0, 3]), &mut || 0).damage, [7, 0, 3], "as it is");
        assert_eq!(mob_skill_damage(&hit, &skill(4, [10, 0, 50]), &mut || 0).damage, [20, 0, 25], "share of the maxima");
        assert_eq!(mob_skill_damage(&hit, &skill(5, [50, 0, 0]), &mut || 0).damage, [40, 0, 0], "share of the current");
        assert_eq!(mob_skill_damage(&hit, &skill(3, [2, 0, 0]), &mut || 0).damage, [1000, 0, 0], "times the monster's own HP");
        // Type 1: A = entry 0's 50 + attack 0 + the skill's 10; P 1: (60 + 0.6) x 1 - 0 = 60.6; no critical (99 against 5).
        let blow = mob_skill_damage(&hit, &skill(1, [10, 4, 5]), &mut || 99);
        assert_eq!(blow, SkillBlow { result: RESULT_HIT, damage: [60, 4, 5] });
        // Type 2: the formula without the skill (50.5), times wDamage[0]; a critical: 50.5 x 1.5 = 75.75, x 2.
        let blow = mob_skill_damage(&hit, &skill(2, [2, 9, 9]), &mut || 0);
        assert_eq!(blow, SkillBlow { result: RESULT_CRITICAL, damage: [150, 0, 0] });
    }

    #[test]
    fn magic_skills_on_players_are_cut_by_level_and_magic_defence() {
        let skill = SkillDef { damage_type: 2, attack_kind: MAGIC as u8, ..Default::default() };
        let mut caster = Fighter { level: 20, int: 50, item_attack: 50, wis: 10, ..Default::default() };
        caster.kinds[MAGIC].attack = 200;
        let mut target = Fighter { level: 20, wis: 10, ..Default::default() };
        target.kinds[MAGIC].defence = 100;
        // P1 clamps to 1, P2 = 100 / (50 + 50) × 100 clamps to 70: 200 less 71 %, no luck, no critical (99).
        let h = PvpSkillHit {
            caster: &caster,
            target: &target,
            defence_element: 0,
            caster_health: [100; 3],
            target_health: [100; 3],
            target_max: [100; 3],
            within_three: true,
            cross_road: false,
        };
        assert_eq!(pvp_skill_damage(&h, &skill, &mut || 99), SkillBlow { result: RESULT_HIT, damage: [58, 0, 0] });
        // Resisted when the roll is below a + b = 1 + 1.95.
        assert!(!pvp_skill_hits(&caster, &target, &skill, &mut || 0));
        assert!(pvp_skill_hits(&caster, &target, &skill, &mut || 2));
        // Percentages of the target's maximum (type 4), and a sure hit by success type 1.
        let percent = SkillDef { damage_type: 4, damage: [10, 0, 0], success_type: 1, success_value: 100, ..Default::default() };
        assert_eq!(pvp_skill_damage(&h, &percent, &mut || 0).damage, [10, 0, 0]);
        assert!(pvp_skill_hits(&caster, &target, &percent, &mut || 99));
    }
}
