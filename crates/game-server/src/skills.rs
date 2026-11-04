//! Skills, part 1 (`docs/original/skills.md`): the definitions, who may learn what, and what passive skills add to a character's
//! numbers. Part 2 (`docs/original/skills-use.md`): who may use a skill, on what, the checks, the costs and the cooldown. Pure
//! functions; the connection and the zone apply them.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::combat::Numbers;
/// A quick slot, stored as the client sends it (`stQuick`).
pub use shaiya_proto::skill::QuickSlot;

/// Highest level a learnt skill has (`CGameData::GetSkillInfo`; level 100 is for monsters and items).
pub const MAX_LEVEL: u8 = 3;
/// The level of the rows monsters and items use (`CGameData::LoadConstMobSkill`, `docs/original/mob-skills.md` section 1).
pub const MOB_LEVEL: u8 = 100;
/// Skills a character can hold (`m_pSkill[256]`).
pub const MAX_SKILLS: usize = 256;
/// Real types the per-weapon-type passive arrays cover (`m_byWeaponDamage[20]`).
const WEAPON_TYPES: usize = 20;

/// `0x0209` results.
pub const LEARNT: u8 = 0;
pub const NO_POINTS: u8 = 1;
pub const LEVEL_TOO_LOW: u8 = 2;
pub const CANNOT_LEARN: u8 = 3;

/// `0x0511`/`0x0517` results (`docs/original/skills-use.md` section 13).
pub const RESULT_AREA: u8 = 4;
pub const RESULT_OUT_OF_RANGE: u8 = 5;
pub const RESULT_NO_ENERGY: u8 = 6;
pub const RESULT_EQUIPMENT: u8 = 7;
pub const RESULT_PREVIOUS: u8 = 8;
pub const RESULT_COOLDOWN: u8 = 9;
pub const RESULT_CANNOT_ATTACK: u8 = 10;
pub const RESULT_WRONG_TARGET: u8 = 11;

/// Kinds (`byTypeEffect`).
pub const PASSIVE: u8 = 0;
pub const ATTACK: u8 = 1;
pub const HEAL: u8 = 2;
pub const DEBUFF: u8 = 3;
pub const BUFF: u8 = 4;
pub const FIELD: u8 = 5;
pub const ONCE: u8 = 6;
/// Target types (`byTargetType`).
pub const TARGET_ONESELF: u8 = 2;
pub const TARGET_ONE: u8 = 3;
pub const TARGET_PARTY: u8 = 4;
pub const TARGET_PARTY_AROUND: u8 = 5;
pub const TARGET_AROUND_ONESELF: u8 = 6;
pub const TARGET_AROUND_TARGET: u8 = 7;
/// Variants (`wType`).
pub const MULTI_HIT: u16 = 22;
pub const DESPERATE: u16 = 23;
pub const NEUTRAL_ELEMENT: u16 = 24;
pub const DRAIN: u16 = 27;
pub const HEAL_HEALTH: u16 = 41;
pub const DISPEL: u16 = 42;
pub const SACRIFICE: u16 = 43;
pub const TRANSFORMATION: u16 = 90;
/// Buffs that keep monsters from seeing the player (no learnable row; not built).
pub const NOT_SEEN: u16 = 89;
/// Fields (`docs/original/skills-special.md` section 8) and one-off skills (section 9).
pub const TRAP: u16 = 101;
pub const FIRE_WALL: u16 = 102;
pub const RESURRECTION: u16 = 121;
pub const SCOUT: u16 = 123;
pub const RETURN: u16 = 125;

/// What the server needs of a `stSkillInfo` (`docs/original/skills.md` section 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillDef {
    pub id: u16,
    pub level: u8,
    /// Who may learn it by race (`byCountry`, section 3).
    pub country: u8,
    /// `byJob`: the jobs 0 to 5 that may learn it.
    pub jobs: [bool; 6],
    /// The lowest mode it needs (`byGrow`).
    pub grow: u8,
    pub points: u8,
    pub required_level: u16,
    /// 0: cannot be learnt (`byTypeShow`).
    pub type_show: u8,
    /// 0: passive (`byTypeEffect`).
    pub type_effect: u8,
    /// The passive's kind (`wType`, the `typedetail` column).
    pub kind: u16,
    /// Real types for masteries (`byWeapon`, 0 = none) and the amount (`byWeaponValue`).
    pub weapons: [u8; 2],
    pub weapon_value: u8,
    /// Bags for the bag passive (`byBag`).
    pub bags: u8,
    /// `stAbility`: (type, value), type 0 = none.
    pub abilities: [(u8, u16); 3],
    /// A skill whose cooldown must be running for this one to be used (`wPrevSkill`, 0 = none).
    pub prev_skill: u16,
    /// 0 melee, 1 ranged, 2 magic (`byTypeAttack`, stored lowered by one).
    pub attack_kind: u8,
    /// The weapon real types 1 to 15 it may be used with, and whether it needs a shield instead (`byNeedWeapon`, the `shield`
    /// column its last entry).
    pub need_weapons: [bool; 15],
    pub need_shield: bool,
    /// SP and MP it costs (`wNeedSP`, `wNeedMP`); 1 is everything the player has.
    pub need_sp: u16,
    pub need_mp: u16,
    /// Casting time in quarter seconds (`byReadyTime`) and the cooldown in seconds (`wResetTime`).
    pub ready_time: u8,
    pub reset_time: u16,
    /// Reach added to the player's (`byAttackRange`), and the radius of an area (`byApplyRange`).
    pub attack_range: u8,
    pub apply_range: u8,
    /// The monster state it inflicts, which monsters may resist (`byStateType`, 1 to 15).
    pub state_type: u8,
    /// The element it attacks with, 0 = the weapon's (`byAttrType`).
    pub element: u8,
    /// How the hit chance is decided (`bySuccessType`, `bySuccessValue`).
    pub success_type: u8,
    pub success_value: u8,
    /// Whom it reaches (`byTargetType`).
    pub target_type: u8,
    /// Hits of a multi-hit (`byMultiAttack`).
    pub multi_attack: u8,
    /// Aggro added with a hit (`wAggro`, the `arrow` column).
    pub aggro: u16,
    /// How the damage is worked out (`byDamageType`) and its HP, SP, MP values (`wDamage`), the additional damage (`wAddDamage`) and
    /// the heal (`wHeal`), each HP, SP, MP.
    pub damage_type: u8,
    pub damage: [u16; 3],
    pub add_damage: [u16; 3],
    pub heal: [u16; 3],
    /// How long an applied skill lasts, in seconds (`dwKeepTime`; stored in hours for `keep_on_death` rows and corrected to seconds).
    pub keep_time: u32,
    /// Damage over time: how it is worked out (`byTimeDamageType`) and its HP, SP, MP values (`wTimeDamage`); the heal over time
    /// (`wTimeHeal`), HP, SP, MP.
    pub time_damage_type: u8,
    pub time_damage: [u16; 3],
    pub time_heal: [u16; 3],
    /// The attack kind a block buff covers (`byDefenseType`, stored lowered by one) and its count or chance (`byDefenseValue`).
    pub defense_type: u8,
    pub defense_value: u8,
    /// A low-HP passive's threshold in percent of max HP (`byLimitHP`).
    pub limit_hp: u8,
    /// Kept through death, counting down in real time even offline (`byDontRemByDeath`, the `fixrange` column).
    pub keep_on_death: bool,
    /// A transformation's shape (`wChangeType`: 100 disguise, 101 polymorph, 102 illusion, 10 to 12 evolution) and the monster whose
    /// fighting numbers it brings (`wChangeLevel`).
    pub change_type: u16,
    pub change_level: u16,
}

impl SkillDef {
    /// `CGameData::LoadConstSkill`'s corrections of a row.
    fn corrected(mut self) -> Self {
        self.weapons = self.weapons.map(|w| w.min(WEAPON_TYPES as u8 - 1));
        for (kind, value) in &mut self.abilities {
            if *kind != 0 && *value == 0 {
                *value = 1;
            }
        }
        match self.kind {
            93 => self.type_effect = 4,
            122 => self.type_effect = 6,
            _ => {}
        }
        self.attack_kind = self.attack_kind.clamp(1, 3) - 1;
        self.defense_type = self.defense_type.clamp(1, 3) - 1;
        if self.element >= 9 {
            self.element = 0;
        }
        if self.keep_on_death {
            self.keep_time = self.keep_time.saturating_mul(3600);
        }
        self
    }

    /// What is still refused with result 10 (deviation, `docs/original/skills-special.md` section 12): fields other than traps and
    /// fire walls, one-off skills other than resurrection, scouting and return (none of the others can be learnt), and the buffs
    /// that keep monsters from seeing (no row).
    pub fn not_built(&self) -> bool {
        match self.type_effect {
            FIELD => !matches!(self.kind, TRAP | FIRE_WALL),
            ONCE => !matches!(self.kind, RESURRECTION | SCOUT | RETURN),
            BUFF => self.kind == NOT_SEEN,
            _ => false,
        }
    }
}

/// A skill named in a setting (`kill_boost_skills`): an id, and a level when only that level counts. The monsters' skills share
/// the players' ids at level 100 (`MOB_LEVEL`), so `200/1` is the kill-boost item's buff and not the monsters' Weakening Curse 200/100.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillMatch {
    pub id: u16,
    pub level: Option<u8>,
}

impl SkillMatch {
    pub fn matches(&self, def: &SkillDef) -> bool {
        def.id == self.id && self.level.is_none_or(|level| def.level == level)
    }
}

/// One entry of `kill_boost_skills`: a skill, and how many times a player kill counts while it is applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KillBoost {
    pub skill: SkillMatch,
    pub times: u32,
}

/// The definitions of levels 1 to 3 by (id, level).
#[derive(Debug, Default)]
pub struct SkillDefs(HashMap<(u16, u8), SkillDef>);

impl SkillDefs {
    /// As the original's loaders: rows of levels 1 to 3 and 100 (the monsters' and items', `LoadConstMobSkill`) with an id, each
    /// corrected (section 1).
    pub fn new(defs: impl IntoIterator<Item = SkillDef>) -> Self {
        let kept = |d: &SkillDef| d.id != 0 && ((1..=MAX_LEVEL).contains(&d.level) || d.level == MOB_LEVEL);
        Self(defs.into_iter().filter(kept).map(|d| ((d.id, d.level), d.corrected())).collect())
    }

    pub fn get(&self, id: u16, level: u8) -> Option<&SkillDef> {
        self.0.get(&(id, level))
    }

    /// These and more rows, loaded as `new` loads them.
    pub fn with(mut self, more: impl IntoIterator<Item = SkillDef>) -> Self {
        self.0.extend(Self::new(more).0);
        self
    }

    /// The level-100 rows: the monsters' and the items' skills (`GetSkillInfo(id, 100)`).
    pub fn level_100(&self) -> impl Iterator<Item = &SkillDef> {
        self.0.values().filter(|d| d.level == MOB_LEVEL)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A learnt skill (`CSkill`; `DB_SKILL_UNIT`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skill {
    pub id: u16,
    pub level: u8,
    /// Its index in the character's list, given in learning order and kept by later levels.
    pub number: u8,
    /// Seconds until it can be used again, as stored (cooldowns stand still while offline).
    pub delay: u16,
}

/// What a learn request needs about the character.
#[derive(Clone, Copy, Debug)]
pub struct Learner {
    pub level: u16,
    pub skill_points: u16,
    /// 0 human, 1 elf, 2 vail, 3 deatheater.
    pub family: u8,
    pub job: u8,
    /// 0 easy, 1 normal, 2 hard, 3 ultimate.
    pub grow: u8,
}

/// The outcome of a learn request: the skill as it now is and its definition's cost, or the `0x0209` result.
pub fn learn<'a>(learner: &Learner, skills: &[Skill], id: u16, level: u8, defs: &'a SkillDefs) -> Result<(Skill, &'a SkillDef), u8> {
    if !(1..=MAX_LEVEL).contains(&level) {
        return Err(CANNOT_LEARN);
    }
    let def = defs.get(id, level).ok_or(CANNOT_LEARN)?;
    let skill = match skills.iter().find(|s| s.id == id) {
        // Only the next level (deviation: the original accepts any higher level).
        Some(known) if known.level + 1 != level => return Err(CANNOT_LEARN),
        Some(known) => Skill { level, delay: 0, ..*known },
        // A new skill starts at level 1 (the original does not answer other levels).
        None if level != 1 => return Err(CANNOT_LEARN),
        None if skills.len() >= MAX_SKILLS => return Err(CANNOT_LEARN),
        None => Skill { id, level, number: next_number(skills), delay: 0 },
    };
    check(learner, def)?;
    Ok((skill, def))
}

/// The number a new skill gets: the count, as in the original; past a gap (should one exist) the first free one.
fn next_number(skills: &[Skill]) -> u8 {
    let taken = |n: u8| skills.iter().any(|s| s.number == n);
    let count = skills.len() as u8;
    if !taken(count) { count } else { (0..=u8::MAX).find(|&n| !taken(n)).unwrap_or(count) }
}

/// `CUser::EnableLearnSkill`, in its order.
fn check(learner: &Learner, def: &SkillDef) -> Result<(), u8> {
    if learner.skill_points < u16::from(def.points) {
        return Err(NO_POINTS);
    }
    if learner.level < def.required_level {
        return Err(LEVEL_TOO_LOW);
    }
    let race_ok = match def.country {
        0 => learner.family == 0,
        1 => learner.family == 1,
        2 => learner.family <= 1,
        3 => learner.family == 3,
        4 => learner.family == 2,
        5 => learner.family >= 2,
        _ => true,
    };
    // The original does not answer a race mismatch (deviation: result 3).
    if !race_ok || !def.jobs.get(usize::from(learner.job)).copied().unwrap_or(false) {
        return Err(CANNOT_LEARN);
    }
    let grow_ok = match learner.grow {
        0 => def.grow == 0,
        1 => def.grow <= 1,
        2 => def.grow != 3,
        _ => true,
    };
    if !grow_ok || def.type_show == 0 {
        return Err(CANNOT_LEARN);
    }
    Ok(())
}

/// What abilities add (`CUser::SetSkillAbility`, section 6). Health is HP, MP, SP; stats STR, DEX, REC, INT, WIS, LUC.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bonuses {
    pub max_health: [i32; 3],
    pub stats: [i32; 6],
    /// Sitting recovery in percent of the maximum, HP, MP, SP (`m_nSitRecHPP`, `MPP`, `SPP`).
    pub sit_recovery_percent: [i32; 3],
    /// Skill cost reductions, SP and MP (`m_nDecSP`, `m_nDecMP`; part 2).
    pub cost_reduction: [i32; 2],
    pub attack_range: i32,
    pub attack_speed: i32,
    pub move_speed: i32,
    pub critical: i32,
    /// `m_nCoolTime` (part 2).
    pub cool_time: i32,
    /// The `add` terms of the melee, ranged and magic numbers (`m_pAttackAdd`).
    pub adds: [Numbers; 3],
    /// Attack kinds switched off (`m_pAttack[kind].nDisable`).
    pub disabled: [i32; 3],
}

const STR: usize = 0;
const DEX: usize = 1;
const REC: usize = 2;
const INT: usize = 3;
const WIS: usize = 4;
const LUC: usize = 5;
const HP: usize = 0;
const MP: usize = 1;
const SP: usize = 2;
/// Max HP, MP or SP per point of REC, WIS or DEX from an ability.
const STAT_HEALTH: i32 = 5;

impl Bonuses {
    /// Adds one ability; `type_effect` 3 leaves the maxima alone (types 1 to 3, and the maxima of 5, 7 and 8).
    pub fn add(&mut self, ability: u8, value: i32, type_effect: u8) {
        let maxima = type_effect != 3;
        let stat = |b: &mut Self, stat: usize, health: Option<usize>| {
            b.stats[stat] += value;
            if let (Some(h), true) = (health, maxima) {
                b.max_health[h] += value * STAT_HEALTH;
            }
        };
        match ability {
            1 if maxima => self.max_health[HP] += value,
            2 if maxima => self.max_health[MP] += value,
            3 if maxima => self.max_health[SP] += value,
            4 => stat(self, STR, None),
            5 => stat(self, REC, Some(HP)),
            6 => stat(self, INT, None),
            7 => stat(self, WIS, Some(MP)),
            8 => stat(self, DEX, Some(SP)),
            9 => stat(self, LUC, None),
            10 => self.sit_recovery_percent[HP] += value,
            11 => self.sit_recovery_percent[SP] += value,
            12 => self.sit_recovery_percent[MP] += value,
            13 => self.cost_reduction[0] += value,
            14 => self.cost_reduction[1] += value,
            15 => self.attack_range += value,
            16 => self.attack_speed += value,
            17 => self.move_speed += value,
            18 => self.critical += value,
            19 => self.cool_time += value,
            20..=22 => self.adds[usize::from(ability - 20)].hit += value,
            23..=25 => self.adds[usize::from(ability - 23)].attack += value,
            26..=28 => self.adds[usize::from(ability - 26)].defence += value,
            29..=31 => self.adds[usize::from(ability - 29)].evasion += value,
            32..=34 => self.disabled[usize::from(ability - 32)] += value,
            // 35 to 41: item-skill flags (premium items), read by `buffs.rs` where built (36, 37, 39); the rest are not read.
            _ => {}
        }
    }
}

impl Bonuses {
    /// Both sets of bonuses together (passive skills and applied skills).
    pub fn plus(mut self, other: &Bonuses) -> Bonuses {
        let add = |a: &mut [i32], b: &[i32]| a.iter_mut().zip(b).for_each(|(x, y)| *x += y);
        add(&mut self.max_health, &other.max_health);
        add(&mut self.stats, &other.stats);
        add(&mut self.sit_recovery_percent, &other.sit_recovery_percent);
        add(&mut self.cost_reduction, &other.cost_reduction);
        add(&mut self.disabled, &other.disabled);
        self.attack_range += other.attack_range;
        self.attack_speed += other.attack_speed;
        self.move_speed += other.move_speed;
        self.critical += other.critical;
        self.cool_time += other.cool_time;
        for (a, b) in self.adds.iter_mut().zip(&other.adds) {
            a.hit += b.hit;
            a.evasion += b.evasion;
            a.attack += b.attack;
            a.defence += b.defence;
        }
        self
    }
}

/// What the passive skills add (`CUser::AddSkillPassive`, section 5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Passives {
    /// Attack and attack speed for a weapon of each real type (`m_byWeaponDamage`, `m_byWeaponSpeed`).
    pub weapon_damage: [i32; WEAPON_TYPES],
    pub weapon_speed: [i32; WEAPON_TYPES],
    /// Defence while a shield is worn (`m_byShieldValue`).
    pub shield: i32,
    /// Usable bags, when a bag passive is learnt (`m_nMaxBag`).
    pub bags: Option<u8>,
    /// `m_nLanguage`; nothing in the server reads it.
    pub language: u8,
    /// Ranged attacks within 3 keep their full damage (`m_nCrossRoad`).
    pub cross_road: i32,
    pub bonuses: Bonuses,
}

impl Passives {
    /// The weapon-type value for an item of `real_type` (0 outside the arrays).
    pub fn weapon(values: &[i32; WEAPON_TYPES], real_type: u8) -> i32 {
        values.get(usize::from(real_type)).copied().unwrap_or(0)
    }
}

/// The passive skills' effects, from scratch (the original adds and removes them one by one; the result is the same while levels
/// are learnt in order). Type 6 (low-HP abilities) is part 3.
pub fn passives(skills: &[Skill], defs: &SkillDefs) -> Passives {
    let mut p = Passives::default();
    for def in skills.iter().filter_map(|s| defs.get(s.id, s.level)).filter(|d| d.type_effect == 0) {
        let value = i32::from(def.weapon_value);
        let weapons = def.weapons.into_iter().filter(|&w| w != 0).map(usize::from).filter(|&w| w < WEAPON_TYPES);
        match def.kind {
            1 => weapons.for_each(|w| p.weapon_damage[w] += value),
            2 => p.bags = Some(def.bags),
            4 => {
                for (ability, value) in def.abilities {
                    p.bonuses.add(ability, i32::from(value), def.type_effect);
                }
            }
            5 => p.language = def.level,
            8 => weapons.for_each(|w| p.weapon_speed[w] += value),
            9 => p.cross_road += 1,
            10 => p.shield += value,
            _ => {}
        }
    }
    p
}

/// Whether the character may use the skill now (`CUser::PacketPC` 0x0511/0x0517, section 2 step 5): shown, its level reached, its
/// job, its mode not above the character's (a plain comparison, unlike learning).
pub fn usable(def: &SkillDef, level: u16, job: u8, grow: u8) -> bool {
    def.type_show != 0
        && (def.required_level == 0 || level >= def.required_level)
        && def.jobs.get(usize::from(job)).copied().unwrap_or(false)
        && def.grow <= grow
}

/// Whom a skill request aims at by the skill's target type (`CUser::ChkTargetMob`, `CUser::ChkTargetUser`, section 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aim {
    Oneself,
    /// The monster or player named in the request, if it qualifies.
    Named,
    /// Result 11.
    Wrong,
}

pub fn aim(def: &SkillDef) -> Aim {
    match def.target_type {
        1 | 2 | 4 | 5 | 6 => Aim::Oneself,
        3 | 7 => Aim::Named,
        _ => Aim::Wrong,
    }
}

/// What the checks need of the caster (section 6).
#[derive(Clone, Copy, Debug, Default)]
pub struct Caster {
    pub sp: u16,
    pub mp: u16,
    /// `m_nDecSP`, `m_nDecMP` in percent.
    pub cost_reduction: [i32; 2],
    /// The real type of the weapon in slot 5, and whether slot 6 holds anything.
    pub weapon: Option<u8>,
    pub shield: bool,
    /// `m_pAttack[kind].nDisable` for melee, ranged and magic.
    pub disabled: [i32; 3],
    /// `m_nCoolTime` in seconds.
    pub cool_time: i32,
}

/// A cost after the reduction (`trunc(need / 100 × reduction)` off, single precision).
pub fn cost(need: u16, reduction: i32) -> u16 {
    let off = (f32::from(need) / 100.0 * reduction as f32) as i32;
    (i32::from(need) - off).clamp(0, i32::from(u16::MAX)) as u16
}

/// Whether a skill whose cooldown ends at `reset` is ready (check 4): with `m_nCoolTime` it comes back that much early, unless
/// the reset time is no more than that many seconds of tick count (the original's guard against going below 0).
fn ready(reset: Option<Instant>, cool_time: i32, now: Instant) -> bool {
    let Some(reset) = reset else { return true };
    if cool_time < 1 {
        return now >= reset;
    }
    match reset.checked_sub(Duration::from_secs(cool_time as u64)) {
        Some(early) => now >= early,
        None => true,
    }
}

/// `CUser::EnableUseSkillMob`/`EnableUseSkillUser` (section 6), in their order, with the refusal of what part 3 builds first.
/// `cooldown` gives when a learnt skill's cooldown ends by skill id; `in_range` is check 7 (true without a target).
pub fn check_use(
    def: &SkillDef,
    caster: &Caster,
    skills: &[Skill],
    cooldown: impl Fn(u16) -> Option<Instant>,
    in_range: bool,
    now: Instant,
) -> Result<(), u8> {
    if def.not_built() {
        return Err(RESULT_CANNOT_ATTACK);
    }
    if caster.sp < cost(def.need_sp, caster.cost_reduction[0]) || caster.mp < cost(def.need_mp, caster.cost_reduction[1]) {
        return Err(RESULT_NO_ENERGY);
    }
    let equipped = if def.need_shield {
        caster.shield
    } else {
        caster.weapon.is_some_and(|t| t >= 1 && def.need_weapons.get(usize::from(t) - 1).copied().unwrap_or(false))
    };
    if !equipped {
        return Err(RESULT_EQUIPMENT);
    }
    if !ready(cooldown(def.id), caster.cool_time, now) {
        return Err(RESULT_COOLDOWN);
    }
    if def.prev_skill != 0 && !skills.iter().any(|s| s.id == def.prev_skill && cooldown(s.id).is_some_and(|reset| now < reset)) {
        return Err(RESULT_PREVIOUS);
    }
    if caster.disabled.get(usize::from(def.attack_kind)).is_some_and(|&d| d != 0) {
        return Err(RESULT_CANNOT_ATTACK);
    }
    if !in_range {
        return Err(RESULT_OUT_OF_RANGE);
    }
    Ok(())
}

/// When the cooldown started now ends (section 7 step 1): with `m_nCoolTime`, long cooldowns are kept (and come back early through
/// the check) and short ones vanish.
pub fn cooldown_end(def: &SkillDef, cool_time: i32, now: Instant) -> Instant {
    if cool_time >= 1 && def.reset_time <= 500 { now } else { now + Duration::from_secs(u64::from(def.reset_time)) }
}

/// What a skill takes of SP and MP (`CUser::SendUseSPMP`, section 7 step 3): each cost after the reduction, a cost of exactly 1 being
/// everything; never more than there is. Returns what is spent, SP then MP.
pub fn spend(def: &SkillDef, sp: u16, mp: u16, reduction: [i32; 2]) -> (u16, u16) {
    let take = |need: u16, reduction: i32, have: u16| match cost(need, reduction) {
        0 => 0,
        1 => have,
        c => c.min(have),
    };
    (take(def.need_sp, reduction[0], sp), take(def.need_mp, reduction[1], mp))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(id: u16, level: u8) -> SkillDef {
        SkillDef {
            id,
            level,
            country: 6,
            jobs: [true; 6],
            points: 2,
            required_level: 1,
            type_show: 1,
            type_effect: 1,
            ..Default::default()
        }
    }

    fn defs() -> SkillDefs {
        let mut mastery = def(10, 1);
        (mastery.type_effect, mastery.kind, mastery.weapons, mastery.weapon_value) = (0, 1, [1, 3], 5);
        let mut mastery2 = def(10, 2);
        (mastery2.type_effect, mastery2.kind, mastery2.weapons, mastery2.weapon_value) = (0, 1, [1, 3], 9);
        let mut bags = def(4, 1);
        (bags.type_effect, bags.kind, bags.bags) = (0, 2, 3);
        let mut ability = def(1, 1);
        (ability.type_effect, ability.kind, ability.abilities) = (0, 4, [(5, 10), (23, 30), (0, 0)]);
        let mut high = def(20, 1);
        high.required_level = 14;
        let mut hidden = def(30, 1);
        hidden.type_show = 0;
        let mut elves = def(31, 1);
        elves.country = 1;
        let mut hard = def(32, 1);
        hard.grow = 3;
        SkillDefs::new([def(2, 1), def(2, 2), def(2, 3), mastery, mastery2, bags, ability, high, hidden, elves, hard, def(5, 100)])
    }

    fn learner() -> Learner {
        Learner { level: 10, skill_points: 5, family: 0, job: 0, grow: 1 }
    }

    #[test]
    fn levels_one_to_three_and_the_monsters_100_are_loaded() {
        assert!(defs().get(5, 100).is_some());
        assert!(SkillDefs::new([def(6, 4), def(7, 0), def(0, 1)]).is_empty());
        assert_eq!(defs().len(), 12);
    }

    #[test]
    fn rows_are_corrected_as_the_original_loads_them() {
        let mut d = def(1, 1);
        (d.weapons, d.abilities, d.kind) = ([25, 3], [(4, 0), (0, 0), (5, 7)], 93);
        let defs = SkillDefs::new([d]);
        let d = defs.get(1, 1).unwrap();
        assert_eq!((d.weapons, d.abilities, d.type_effect), ([19, 3], [(4, 1), (0, 0), (5, 7)], 4));
    }

    #[test]
    fn new_skills_start_at_level_one_and_take_the_next_number() {
        let defs = defs();
        let known = [Skill { id: 1, level: 1, number: 0, delay: 0 }];
        let (skill, cost) = learn(&learner(), &known, 2, 1, &defs).unwrap();
        assert_eq!(skill, Skill { id: 2, level: 1, number: 1, delay: 0 });
        assert_eq!(cost.points, 2);
        assert_eq!(learn(&learner(), &known, 2, 2, &defs), Err(CANNOT_LEARN), "a new skill at level 2");
        assert_eq!(learn(&learner(), &known, 2, 4, &defs), Err(CANNOT_LEARN));
        assert_eq!(learn(&learner(), &known, 99, 1, &defs), Err(CANNOT_LEARN), "no definition");
    }

    #[test]
    fn higher_levels_keep_the_number_and_must_be_the_next() {
        let defs = defs();
        let known = [Skill { id: 7, level: 1, number: 0, delay: 0 }, Skill { id: 2, level: 1, number: 1, delay: 30 }];
        let (skill, _) = learn(&learner(), &known, 2, 2, &defs).unwrap();
        assert_eq!(skill, Skill { id: 2, level: 2, number: 1, delay: 0 }, "cooldown ready at once");
        assert_eq!(learn(&learner(), &known, 2, 3, &defs), Err(CANNOT_LEARN), "skipping a level");
        assert_eq!(learn(&learner(), &known, 2, 1, &defs), Err(CANNOT_LEARN), "already known");
    }

    #[test]
    fn checks_in_the_original_order() {
        let defs = defs();
        let poor = Learner { skill_points: 1, level: 1, ..learner() };
        assert_eq!(learn(&poor, &[], 20, 1, &defs), Err(NO_POINTS), "points before level");
        assert_eq!(learn(&Learner { level: 13, ..learner() }, &[], 20, 1, &defs), Err(LEVEL_TOO_LOW));
        assert!(learn(&Learner { level: 14, ..learner() }, &[], 20, 1, &defs).is_ok());
        assert_eq!(learn(&learner(), &[], 30, 1, &defs), Err(CANNOT_LEARN), "type show 0");
        assert_eq!(learn(&learner(), &[], 31, 1, &defs), Err(CANNOT_LEARN), "elves only");
        assert!(learn(&Learner { family: 1, ..learner() }, &[], 31, 1, &defs).is_ok());
        assert_eq!(learn(&learner(), &[], 32, 1, &defs), Err(CANNOT_LEARN), "ultimate only");
        assert!(learn(&Learner { grow: 3, ..learner() }, &[], 32, 1, &defs).is_ok());
    }

    #[test]
    fn races_jobs_and_modes() {
        let mut d = def(1, 1);
        for (country, families) in [(0, [0].as_slice()), (1, &[1]), (2, &[0, 1]), (3, &[3]), (4, &[2]), (5, &[2, 3]), (6, &[0, 1, 2, 3])] {
            d.country = country;
            for family in 0..4 {
                let ok = check(&Learner { family, ..learner() }, &d).is_ok();
                assert_eq!(ok, families.contains(&family), "country {country} family {family}");
            }
        }
        d.country = 6;
        d.jobs = [false, true, false, false, false, false];
        assert_eq!(check(&learner(), &d), Err(CANNOT_LEARN));
        assert!(check(&Learner { job: 1, ..learner() }, &d).is_ok());
        d.jobs = [true; 6];
        for (grow, allowed) in
            [(0, [true, false, false, false]), (1, [true, true, false, false]), (2, [true, true, true, false]), (3, [true; 4])]
        {
            for skill_grow in 0..4 {
                d.grow = skill_grow;
                assert_eq!(
                    check(&Learner { grow, ..learner() }, &d).is_ok(),
                    allowed[usize::from(skill_grow)],
                    "mode {grow} skill {skill_grow}"
                );
            }
        }
    }

    #[test]
    fn passives_by_kind() {
        let defs = defs();
        let skills = [
            Skill { id: 10, level: 2, number: 0, delay: 0 },
            Skill { id: 4, level: 1, number: 1, delay: 0 },
            Skill { id: 1, level: 1, number: 2, delay: 0 },
            Skill { id: 2, level: 1, number: 3, delay: 0 },
        ];
        let p = passives(&skills, &defs);
        assert_eq!((p.weapon_damage[1], p.weapon_damage[3], p.weapon_damage[2]), (9, 9, 0), "only the current level counts");
        assert_eq!(p.bags, Some(3));
        assert_eq!(p.bonuses.stats[REC], 10);
        assert_eq!(p.bonuses.max_health[HP], 50, "REC adds 5 HP a point");
        assert_eq!(p.bonuses.adds[0].attack, 30);
        assert_eq!(passives(&skills[3..], &defs), Passives::default(), "active skills add nothing");
    }

    /// A melee attack on one monster with a sword (real type 1): 10 SP, 4 s cooldown.
    fn strike() -> SkillDef {
        let mut need_weapons = [false; 15];
        need_weapons[0] = true;
        SkillDef { type_effect: ATTACK, target_type: TARGET_ONE, kind: 21, need_weapons, need_sp: 10, reset_time: 4, ..def(5, 1) }
    }

    fn caster() -> Caster {
        Caster { sp: 10, mp: 0, weapon: Some(1), ..Default::default() }
    }

    #[test]
    fn the_attack_kind_is_stored_lowered_by_one() {
        let kinds: Vec<u8> =
            [0, 1, 2, 3, 7].map(|k| SkillDefs::new([SkillDef { attack_kind: k, ..def(1, 1) }]).get(1, 1).unwrap().attack_kind).into();
        assert_eq!(kinds, [0, 0, 1, 2, 2]);
    }

    #[test]
    fn who_may_use_a_skill_and_on_what() {
        let d = SkillDef { required_level: 5, grow: 2, jobs: [true, false, false, false, false, false], ..strike() };
        assert!(usable(&d, 5, 0, 2));
        assert!(!usable(&d, 4, 0, 3), "level");
        assert!(!usable(&d, 5, 1, 3), "job");
        assert!(!usable(&d, 5, 0, 1), "mode");
        assert!(!usable(&SkillDef { type_show: 0, ..d }, 5, 0, 2));
        let aims = [0, 1, 2, 3, 4, 5, 6, 7].map(|target_type| aim(&SkillDef { target_type, ..d }));
        use Aim::*;
        assert_eq!(aims, [Wrong, Oneself, Oneself, Named, Oneself, Oneself, Oneself, Named]);
        // Transformations: as their target type says; the request decides the rest (`docs/original/skills-special.md` section 5).
        assert_eq!(aim(&SkillDef { kind: TRANSFORMATION, target_type: 3, ..d }), Named);
    }

    #[test]
    fn checks_come_in_the_original_order() {
        let now = Instant::now();
        let none = |_| None;
        let d = strike();
        assert_eq!(check_use(&d, &caster(), &[], none, true, now), Ok(()));
        assert_eq!(
            check_use(&d, &Caster { sp: 9, weapon: None, ..caster() }, &[], none, false, now),
            Err(RESULT_NO_ENERGY),
            "energy first"
        );
        assert_eq!(check_use(&SkillDef { need_mp: 1, ..d }, &caster(), &[], none, true, now), Err(RESULT_NO_ENERGY), "MP");
        assert_eq!(check_use(&d, &Caster { weapon: Some(2), ..caster() }, &[], none, false, now), Err(RESULT_EQUIPMENT));
        assert_eq!(check_use(&SkillDef { need_shield: true, ..d }, &caster(), &[], none, true, now), Err(RESULT_EQUIPMENT));
        assert_eq!(
            check_use(&SkillDef { need_shield: true, ..d }, &Caster { shield: true, weapon: None, ..caster() }, &[], none, true, now),
            Ok(())
        );
        let cooling = |id| (id == 5).then_some(now + Duration::from_secs(1));
        assert_eq!(check_use(&d, &caster(), &[], cooling, false, now), Err(RESULT_COOLDOWN));
        // A combo: skill 5 must have been used just before (its cooldown running).
        let combo = SkillDef { id: 6, prev_skill: 5, ..d };
        let known = [Skill { id: 5, level: 1, number: 0, delay: 0 }];
        assert_eq!(check_use(&combo, &caster(), &known, none, false, now), Err(RESULT_PREVIOUS));
        assert_eq!(check_use(&combo, &caster(), &[], cooling, true, now), Err(RESULT_PREVIOUS), "not known");
        assert_eq!(check_use(&combo, &caster(), &known, cooling, true, now), Ok(()));
        assert_eq!(check_use(&d, &Caster { disabled: [1, 0, 0], ..caster() }, &[], none, false, now), Err(RESULT_CANNOT_ATTACK));
        assert_eq!(check_use(&d, &caster(), &[], none, false, now), Err(RESULT_OUT_OF_RANGE));
        // Not built: refused before anything else (other fields and one-off skills, not being seen by monsters).
        for (type_effect, kind) in [(FIELD, 103), (ONCE, 122), (ONCE, 124), (BUFF, NOT_SEEN)] {
            let d = SkillDef { type_effect, kind, ..d };
            assert_eq!(check_use(&d, &Caster { sp: 0, ..caster() }, &[], none, true, now), Err(RESULT_CANNOT_ATTACK));
        }
        // Buffs, dispel, hiding, transformations, finding the hidden, fields and the built one-off skills go through the checks.
        let built = [(BUFF, 82), (HEAL, DISPEL), (BUFF, 88), (BUFF, TRANSFORMATION), (BUFF, 93), (FIELD, TRAP), (FIELD, FIRE_WALL)];
        for (type_effect, kind) in built.into_iter().chain([RESURRECTION, SCOUT, RETURN].map(|k| (ONCE, k))) {
            let d = SkillDef { type_effect, kind, ..d };
            assert_eq!(check_use(&d, &Caster { sp: 0, ..caster() }, &[], none, true, now), Err(RESULT_NO_ENERGY));
        }
    }

    #[test]
    fn cooldowns_and_the_cool_time_ability() {
        let now = Instant::now();
        let d = strike();
        assert_eq!(cooldown_end(&d, 0, now), now + Duration::from_secs(4));
        assert_eq!(cooldown_end(&d, 3, now), now, "short cooldowns vanish");
        let long = SkillDef { reset_time: 600, ..d };
        assert_eq!(cooldown_end(&long, 3, now), now + Duration::from_secs(600));
        // ... and come back 3 s early.
        let end = Some(now + Duration::from_secs(600));
        assert!(!ready(end, 0, now + Duration::from_secs(597)));
        assert!(ready(end, 3, now + Duration::from_secs(597)));
        assert!(!ready(end, 3, now + Duration::from_secs(596)));
        assert!(ready(None, 0, now));
    }

    #[test]
    fn costs_with_reductions_and_everything() {
        assert_eq!(cost(30, 0), 30);
        // 30 / 100 * 15 = 4.5: 4 off.
        assert_eq!(cost(30, 15), 26);
        let d = SkillDef { need_sp: 30, need_mp: 1, ..strike() };
        assert_eq!(spend(&d, 100, 77, [15, 0]), (26, 77), "a cost of 1 is all the MP");
        assert_eq!(spend(&d, 20, 0, [0, 0]), (20, 0), "never more than there is");
        assert_eq!(spend(&SkillDef { need_sp: 0, need_mp: 0, ..d }, 20, 20, [0, 0]), (0, 0));
    }

    #[test]
    fn abilities_map_to_their_numbers() {
        let mut b = Bonuses::default();
        for ability in 1..=34 {
            b.add(ability, 1, 0);
        }
        assert_eq!(b.max_health, [1 + 5, 1 + 5, 1 + 5]);
        assert_eq!(b.stats, [1; 6]);
        assert_eq!(b.sit_recovery_percent, [1; 3]);
        assert_eq!((b.attack_range, b.attack_speed, b.move_speed, b.critical, b.cool_time), (1, 1, 1, 1, 1));
        assert_eq!(b.adds, [Numbers { hit: 1, evasion: 1, attack: 1, defence: 1 }; 3]);
        let mut no_maxima = Bonuses::default();
        no_maxima.add(1, 100, 3);
        no_maxima.add(5, 2, 3);
        assert_eq!((no_maxima.max_health, no_maxima.stats[REC]), ([0; 3], 2));
    }
}
