//! Skills, part 3 (`docs/original/skills-buffs.md`): the skills applied to a player (buffs, the low-HP passives) and the debuffs
//! applied to monsters, how they come and go, and what they add up to; part 4 (`docs/original/skills-special.md`): the shape that
//! hiding and transformations give. Pure functions; the zone keeps the lists and sends the packets.

use std::time::{Duration, Instant};

use shaiya_proto::skill::AppliedEntry;

use crate::combat::Numbers;
use crate::skills::{BUFF, Bonuses, DEBUFF, PASSIVE, SkillDef, SkillDefs};

/// Buff variants (`wType`, section 5).
pub const TAUNT_AROUND: u16 = 69;
pub const HEAL_OVER_TIME: u16 = 81;
pub const ABILITIES: u16 = 82;
pub const MIRROR: u16 = 83;
/// Fire Thorns: damage around the player every three seconds (part 4 section 7).
pub const FIRE_THORNS: u16 = 84;
pub const BLOCK_COUNT: u16 = 85;
pub const BLOCK_CHANCE: u16 = 86;
pub const GIVE_UP: u16 = 87;
pub const HIDE: u16 = 88;
pub const TRANSFORMATION: u16 = 90;
pub const ABILITIES_TOO: u16 = 91;
pub const BLOCK_ALL: u16 = 92;
pub const ATTACK_ELEMENT: u16 = 94;
pub const DEFENCE_ELEMENT: u16 = 95;
/// Finding the hidden (part 4 section 4): a buff after loading, whatever the data says.
pub const DETECT: u16 = 93;
/// The low-HP passives (`wType` 6 of kind 0, section 11).
pub const LOW_HP: u16 = 6;
/// Debuff variants on monsters (section 12).
pub const DAMAGE_OVER_TIME: u16 = 61;
pub const MOB_ABILITIES: u16 = 62;
pub const DEATH_SENTENCE: u16 = 63;
pub const STUN: u16 = 64;
pub const STOP: u16 = 65;
pub const SLEEP: u16 = 66;
pub const MOB_ABILITIES_TOO: u16 = 67;
pub const NO_ELEMENT: u16 = 68;
pub const TAUNT: u16 = 72;

/// Levels of monster and item skills, kept apart from levels 1 to 3 of the same id.
const LEVEL_100: u8 = crate::skills::MOB_LEVEL;
/// The skill that excludes hiding and transformations when it is a `wType` 82 buff (section 3 step 2).
const EXCLUSIVE_SKILL: u16 = 114;
/// The abilities of the warehouse item flags (Warehouse Recall, Double Warehouse).
const BANK_ANYWHERE: u8 = 39;
const BANK_SECOND_HALF: u8 = 43;
const NO_WEAR: u8 = 36;
const NO_ITEM_LOSS: u8 = 37;
/// The other item flags (`docs/original/item-flags.md` section 1): EXP stones, no experience lost, the charms (amulet levels 1 to 3),
/// gold, Continuous Res Runes.
const EXP_UP: u8 = 35;
const NO_EXP_LOSS: u8 = 38;
const CHARMS: std::ops::RangeInclusive<u8> = 40..=42;
const RED_PHOENIX: u8 = 42;
const MOB_GOLD: u8 = 44;
const REBIRTH: u8 = 46;
/// Shapes (`m_byShape`, part 4 section 2): hidden, and the transformations that are not saved (a disguise in the original; an
/// illusion too here, as its monster is not kept: deviation).
pub const SHAPE_HIDDEN: u8 = 13;
pub const SHAPE_DISGUISE: u16 = 100;
pub const SHAPE_POLYMORPH: u16 = 101;
pub const SHAPE_ILLUSION: u16 = 102;
/// Evolution: the transformation that fighting does not end at levels 1 to 3 (`CUser::RemApplySkillToMOB`).
pub const EVOLUTION: u16 = 127;
/// A low-HP passive lasts 30 s and takes effect at most every 180 s (`CUser::ChkHPAbilityApply`).
pub const LOW_HP_DURATION: Duration = Duration::from_secs(30);
const LOW_HP_REST: Duration = Duration::from_secs(180);
/// Loaded skills get 10 s more (`CUser::AddApplySkill(id, level, delay)`).
const LOAD_GRACE: u64 = 10;
/// The map where some party buffs do not take (`EnableApplySkillByMap`).
const RESTRICTED_MAP: u16 = 18;
/// Damage over time and the heal over time count in units of 3 s.
const TICK_MS: f64 = 3000.0;
/// Monster speeds before any debuff (`CMob::Init`), and the speed that means "cannot".
const MOB_ATTACK_SPEED: i32 = 3;
const MOB_MOVE_SPEED: i32 = 2;
pub const CANNOT: u8 = 255;

/// One skill applied to a player (`CSkill` in `m_applySkill`, section 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Applied {
    /// `m_dwID`: names it to the client (`0x050D`, `0x050E`, the cancel request).
    pub key: u32,
    pub def: SkillDef,
    /// When it was applied, or a heal over time last paid (`m_dwPrevTick`), and when it ends (`m_dwEndTime`).
    pub started: Instant,
    pub ends: Instant,
    /// Who applied it (0 when loaded), and whether that is a player (a monster otherwise): a debuff's damage over time is that
    /// attacker's (`docs/original/pvp-2.md` section 7).
    pub attacker: u32,
    pub by_player: bool,
    /// Blocks left of a block-count buff (`nBlockValue`).
    pub blocks_left: i32,
    /// A transformation's monster (`m_nShapeType`): the copied one for an illusion, else `wChangeLevel`.
    pub mob: u16,
}

impl Applied {
    fn new(key: u32, def: &SkillDef, attacker: u32, started: Instant, ends: Instant) -> Self {
        Self { key, def: *def, started, ends, attacker, by_player: false, blocks_left: i32::from(def.defense_value), mob: def.change_level }
    }

    /// Who applied it as the damage record names attackers: `(cp::KILLER_PLAYER or cp::KILLER_MOB, id)`.
    pub fn attacker_of(&self) -> (u8, u32) {
        let kind = if self.by_player { shaiya_proto::combat::KILLER_PLAYER } else { shaiya_proto::combat::KILLER_MOB };
        (kind, self.attacker)
    }

    fn is(&self, type_effect: u8, kind: u16) -> bool {
        self.def.type_effect == type_effect && self.def.kind == kind
    }

    /// Seconds left, whole, 0 when past.
    pub fn seconds_left(&self, now: Instant) -> u32 {
        self.ends.saturating_duration_since(now).as_secs().min(u64::from(u32::MAX)) as u32
    }
}

/// When a skill applied now ends: `dwKeepTime` seconds on (in hours in the data for `byDontRemByDeath` rows, already corrected).
fn end_of(def: &SkillDef, now: Instant) -> Instant {
    now + Duration::from_secs(u64::from(def.keep_time))
}

/// Not applied at all on map 18 (`EnableApplySkillByMap`).
pub fn refused_on_map(map: u16, def: &SkillDef) -> bool {
    map == RESTRICTED_MAP
        && match def.id {
            111 | 112 | 114 | 116 | 117 | 120 | 121 | 123 | 127 => def.level > 1 && def.level != LEVEL_100,
            124 | 175 | 181 | 185 => true,
            _ => false,
        }
}

/// What applying a skill did: entries taken off on the way (each gets `0x050E` and its end, section 6), the key to send
/// `0x050D` with (`None` when nothing was applied), and whether it is a new entry (a refresh does not add the effect again).
#[derive(Debug, Default, PartialEq)]
pub struct Applying {
    pub ended: Vec<Applied>,
    pub set: Option<u32>,
    pub fresh: bool,
}

/// What a block check did: whether the attack is blocked, and a block-count buff used up by it.
#[derive(Debug, Default, PartialEq)]
pub struct Block {
    pub blocked: bool,
    pub used_up: Option<Applied>,
}

/// What the applied skills add up to (section 5), computed from scratch like the passives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuffEffects {
    /// Abilities of `wType` 82 and 91 buffs, of an active low-HP passive, and (taken off) of debuffs.
    pub bonuses: Bonuses,
    /// `m_bBlockAll`.
    pub block_all: bool,
    /// `m_nGiveupSPMP`: 1 SP, 2 MP, 3 SP then MP pay for HP damage first.
    pub give_up: u8,
    /// Elements that replace the equipment's (`wType` 94 and 95).
    pub attack_element: Option<u8>,
    pub defence_element: Option<u8>,
    /// The shape (`m_byShape`: 0, hidden 13, or a transformation's `wChangeType`) and a transformation's monster (`m_nShapeType`).
    pub shape: u8,
    pub shape_mob: u16,
    /// Debuffs from monsters (`docs/original/mob-skills.md` section 8): stunned, stopped, asleep (`m_nStun`, `m_nStop`,
    /// `m_nSleep`), rooted by a polymorph (`m_nTransFormFixMob`), no defence element (`wType` 68).
    pub stun: bool,
    pub stop: bool,
    pub sleep: bool,
    pub rooted: bool,
    pub no_element: bool,
    /// The item flags of the warehouse (`docs/original/warehouse.md` section 3): ability 39 `m_bSkillEnableBank` (the Warehouse
    /// Recall Rune: the warehouse anywhere) and 43 `m_bSkillBank2X` (Double Warehouse: slots 120 to 239).
    pub bank_anywhere: bool,
    pub bank_second_half: bool,
    /// Ability 36 `m_bSkillNDQuality` (Eternal Endurance): equipment does not wear (`docs/original/repair.md` section 2).
    pub no_wear: bool,
    /// Ability 37 `m_bSkillNDItem` (Prevent Item Drop): nothing is lost to a player killer (`docs/original/pvp.md` section 6.2).
    pub no_item_loss: bool,
    /// `docs/original/item-flags.md`: ability 35 `m_bSkillGetExpUP2X` (EXP stones), 38 `m_bSkillNDExp` (no experience lost at death),
    /// the charm level `m_bSkillAmulet` (40 to 42: 1 to 3, the highest applied), 44 `m_bSkillMobGold` (the item's percent, set not
    /// added) and 46 `m_bSkillRebirth` (Continuous Res Rune).
    pub exp_up: bool,
    /// Premium's `m_bSkillGetExpUP15X` (billing types 2 and 3, `docs/original/item-mall.md` section 7).
    pub exp_premium: bool,
    pub no_exp_loss: bool,
    pub amulet: u8,
    pub mob_gold: i32,
    pub rebirth: bool,
}

impl BuffEffects {
    /// The experience gain's factor (`CUser::AddExpFromUser`, `docs/original/item-flags.md` section 2): 2.0 with an EXP stone
    /// (deviation 1: the original's 1.5 without premium), 1.5 with premium alone, 2.0 with both as the original; + 0.2 with a charm.
    pub fn exp_factor(&self) -> f64 {
        let factor = match (self.exp_up, self.exp_premium) {
            (true, _) => 2.0,
            (false, true) => 1.5,
            (false, false) => 1.0,
        };
        if self.amulet > 0 { factor + 0.2 } else { factor }
    }

    /// The experience lost at this death, in percent (`m_nDropExp`, `CUser::Death`, section 3): 5; 0 with no-loss; 1 with a charm of
    /// level 1 or 2, 0 with level 3.
    pub fn death_exp_percent(&self) -> u8 {
        match (self.no_exp_loss, self.amulet) {
            (true, _) | (false, 3) => 0,
            (false, 1 | 2) => 1,
            _ => 5,
        }
    }

    /// The gold bonus in percent (`CUser::ItemGet`, `CMob::ItemDrop`, section 4): the item's, + 20 with a charm of level 2, + 50 with 3.
    pub fn gold_percent(&self) -> i32 {
        self.mob_gold
            + match self.amulet {
                2 => 20,
                3 => 50,
                _ => 0,
            }
    }

    /// `amount + trunc(p / 100 × amount)` with the gold bonus `p` (section 4), as the original's f32 arithmetic; never below `amount`.
    pub fn with_gold_bonus(&self, amount: u32) -> u32 {
        let p = self.gold_percent();
        if p <= 0 {
            return amount;
        }
        let bonus = (p as f32 / 100.0 * amount as f32) as i64;
        amount.saturating_add(u32::try_from(bonus.max(0)).unwrap_or(u32::MAX))
    }
}

impl BuffEffects {
    /// `CUser::GetAttackSpeed`, `GetMoveSpeed` (section 8): attack 255 while asleep, stunned or rooted; moving 255 while stopped,
    /// asleep or stunned, 0 while rooted; otherwise `speeds` as they are.
    pub fn speeds(&self, (attack, movement): (u8, u8)) -> (u8, u8) {
        let attack = if self.sleep || self.stun || self.rooted { CANNOT } else { attack };
        let movement = if self.stop || self.sleep || self.stun {
            CANNOT
        } else if self.rooted {
            0
        } else {
            movement
        };
        (attack, movement)
    }
}

/// A saved applied skill (`DB_SKILL_APPLY_UNIT`): seconds left when saved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SavedApplied {
    pub skill_id: u16,
    pub level: u8,
    pub seconds: u32,
}

/// A player's applied skills and its low-HP passive (`m_applySkill`, `m_pHPAbility`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerBuffs {
    /// In the order applied.
    pub list: Vec<Applied>,
    /// The low-HP passive learnt, if any, and when it last took effect (`m_pHPAbility`).
    pub low_hp: Option<SkillDef>,
    pub low_hp_last: Option<Instant>,
    /// The one-off protections of cure items (`m_bBlockDying`, `m_bBlockDeath`; `docs/original/item-use.md` section 5): a monster
    /// skill of state 7 or 8 misses once. Not saved.
    pub block_dying: bool,
    pub block_death: bool,
    /// The account's premium (billing type, `CUser::BillingApply`, `docs/original/item-mall.md` section 7), set at world entry and
    /// kept until the character leaves the world. Not saved.
    pub premium: u8,
}

impl PlayerBuffs {
    /// `CUser::AddApplySkill` (section 3), after the map rule. `key` gives a new key.
    pub fn apply(&mut self, def: &SkillDef, attacker: u32, now: Instant, key: &mut impl FnMut() -> u32) -> Applying {
        let mut ended = Vec::new();
        let exclusive = |d: &SkillDef| d.kind == HIDE || d.kind == TRANSFORMATION || (d.id == EXCLUSIVE_SKILL && d.kind == ABILITIES);
        if exclusive(def)
            && let Some(i) = self.list.iter().position(|a| exclusive(&a.def))
        {
            ended.push(self.list.remove(i));
        }
        let level_100 = def.level == LEVEL_100;
        if let Some(i) = self.list.iter().position(|a| a.def.id == def.id && (a.def.level == LEVEL_100) == level_100) {
            let old = &mut self.list[i];
            if old.def.level == def.level {
                old.attacker = attacker;
                old.started = now;
                old.ends = end_of(def, now);
                return Applying { ended, set: Some(old.key), fresh: false };
            }
            if def.level <= old.def.level {
                return Applying { ended, set: None, fresh: false };
            }
            ended.push(self.list.remove(i));
        }
        // One attack-element buff at a time, the newest (section 5.1).
        if def.type_effect == BUFF && def.kind == ATTACK_ELEMENT && def.element < 9 {
            let (other, kept): (Vec<_>, Vec<_>) =
                self.list.drain(..).partition(|a| a.is(BUFF, ATTACK_ELEMENT) && a.def.element != def.element);
            self.list = kept;
            ended.extend(other);
        }
        let new = Applied::new(key(), def, attacker, now, end_of(def, now));
        self.list.push(new);
        Applying { ended, set: Some(new.key), fresh: true }
    }

    /// Whether the entry `key` was applied by a player or a monster (`CUser::AddApplySkill`'s kind).
    pub fn mark_attacker(&mut self, key: u32, by_player: bool) {
        if let Some(a) = self.list.iter_mut().find(|a| a.key == key) {
            a.by_player = by_player;
        }
    }

    /// The entries whose end has come (`CUser::ChkEndTimeSkill`, section 6), taken off.
    pub fn expire(&mut self, now: Instant) -> Vec<Applied> {
        self.take(|a| a.ends <= now)
    }

    /// The buff with that key, taken off (section 7); other kinds stay.
    pub fn cancel(&mut self, key: u32) -> Option<Applied> {
        let i = self.list.iter().position(|a| a.key == key && a.def.type_effect == BUFF)?;
        Some(self.list.remove(i))
    }

    /// What death takes off (section 8): everything not kept through death. A death that costs no experience (`no_exp_loss`,
    /// `m_nDropExp` −1: a duel, a guild war, the flag) also keeps the skills above id 200 at a level other than 100 (the item
    /// buffs and the newer player skills; `docs/original/skills-buffs.md` section 16, the test at 0x48805a).
    pub fn clear_on_death(&mut self, no_exp_loss: bool) -> Vec<Applied> {
        self.take(|a| !a.def.keep_on_death && !(no_exp_loss && a.def.id > 200 && a.def.level != 100))
    }

    /// A dispel of `level` (section 9).
    pub fn dispel(&mut self, level: u8) -> Vec<Applied> {
        self.take(|a| {
            let state = a.def.state_type;
            match level {
                1 => (9..=15).contains(&state),
                2 => (9..=15).contains(&state) || (1..=6).contains(&state),
                _ => a.def.type_effect == DEBUFF,
            }
        })
    }

    /// Whether any debuff is applied (`CUser::PvPHaveDebuff`, `docs/original/duel.md` section 3.1).
    pub fn has_debuff(&self) -> bool {
        self.list.iter().any(|a| a.def.type_effect == DEBUFF)
    }

    /// A cure item (`CUser::RemStateSkill(state…)`, `RemStateSkillDebuffAll`): the applied skills of those state types, or every
    /// debuff, taken off.
    pub fn cure(&mut self, states: &[u8], all_debuffs: bool) -> Vec<Applied> {
        self.take(|a| states.contains(&a.def.state_type) || (all_debuffs && a.def.type_effect == DEBUFF))
    }

    /// `F80E` (`docs/original/gm-commands-2.md` deviation 10): every applied skill but the ones that survive death.
    pub fn take_all_but_kept(&mut self) -> Vec<Applied> {
        self.take(|a| !a.def.keep_on_death)
    }

    /// A monster skill of `state` meets a protection (`CMob::AttackSuccess`, `docs/original/mob-skills.md` section 7 step 2): it
    /// misses, and the protection is used up.
    pub fn protected(&mut self, state: u8) -> bool {
        let protection = match state {
            7 => &mut self.block_dying,
            8 => &mut self.block_death,
            _ => return false,
        };
        std::mem::replace(protection, false)
    }

    /// Whether a buff item's skill is refused (`CUser::UseItemSkill`, `docs/original/item-use.md` section 7): it has an ability
    /// whose item flag an applied skill already sets, a charm no stronger than the one in effect, or the same skill is applied at
    /// a higher level.
    pub fn item_skill_refused(&self, def: &SkillDef) -> bool {
        const FLAGS: [u8; 9] = [35, 36, 37, 38, 39, 43, 44, 45, 46];
        let has = |d: &SkillDef, ability: u8| d.abilities.iter().any(|&(t, v)| t == ability && v != 0);
        let amulet = |d: &SkillDef| (40..=42).filter(|&t| has(d, t)).map(|t| t - 39).max().unwrap_or(0);
        let applied = || self.list.iter().filter(|a| a.def.type_effect == BUFF && matches!(a.def.kind, ABILITIES | ABILITIES_TOO));
        if FLAGS.iter().any(|&flag| has(def, flag) && applied().any(|a| has(&a.def, flag))) {
            return true;
        }
        let wanted = amulet(def);
        if wanted != 0 && applied().map(|a| amulet(&a.def)).max().unwrap_or(0) >= wanted {
            return true;
        }
        self.list.iter().any(|a| a.def.type_effect == BUFF && a.def.id == def.id && a.def.level > def.level)
    }

    /// The monster of the transformation with that key (an illusion's copied monster).
    pub fn set_mob(&mut self, key: u32, mob: u16) {
        if let Some(a) = self.list.iter_mut().find(|a| a.key == key) {
            a.mob = mob;
        }
    }

    /// Attacking (`CUser::RemApplySkillToMOB`, part 4 section 6): the first hiding or transformation in the list ends, except
    /// Evolution at levels 1 to 3 (when not a polymorph), which fighting does not end.
    pub fn end_shape_for_attack(&mut self) -> Option<Applied> {
        let i = self.list.iter().position(|a| a.def.kind == HIDE || a.def.kind == TRANSFORMATION)?;
        let d = &self.list[i].def;
        if d.kind == TRANSFORMATION && d.id == EVOLUTION && d.level != LEVEL_100 && d.change_type != SHAPE_POLYMORPH {
            return None;
        }
        Some(self.list.remove(i))
    }

    /// Waking (`CUser::RemApplySkillSleep`, `docs/original/mob-skills.md` section 9): every sleep debuff goes, without its end.
    pub fn wake(&mut self) -> Vec<Applied> {
        self.take(|a| a.is(DEBUFF, SLEEP))
    }

    /// Being found (`CUser::RemApplySkillHide`, part 4 section 6): the first hiding ends.
    pub fn reveal(&mut self) -> Option<Applied> {
        let i = self.list.iter().position(|a| a.def.kind == HIDE)?;
        Some(self.list.remove(i))
    }

    fn take(&mut self, mut gone: impl FnMut(&Applied) -> bool) -> Vec<Applied> {
        let (out, kept): (Vec<_>, Vec<_>) = self.list.drain(..).partition(|a| gone(a));
        self.list = kept;
        out
    }

    /// What the list adds up to.
    pub fn effects(&self) -> BuffEffects {
        let mut e = BuffEffects::default();
        // CUser::BillingApply: type 1 the warehouse's second half; 2 and 3 no wear, 1.5X experience and the second half.
        match self.premium {
            1 => e.bank_second_half = true,
            2 | 3 => {
                e.bank_second_half = true;
                e.no_wear = true;
                e.exp_premium = true;
            }
            _ => {}
        }
        for a in &self.list {
            let d = &a.def;
            let sign = match (d.type_effect, d.kind) {
                (BUFF, ABILITIES | ABILITIES_TOO) | (PASSIVE, LOW_HP) => 1,
                (DEBUFF, MOB_ABILITIES | MOB_ABILITIES_TOO) => -1,
                _ => 0,
            };
            if sign != 0 {
                for (ability, value) in d.abilities {
                    e.bonuses.add(ability, sign * i32::from(value), d.type_effect);
                }
            }
            if sign == 1 {
                let has = |ability: u8| d.abilities.iter().any(|&(t, v)| t == ability && v != 0);
                e.bank_anywhere |= has(BANK_ANYWHERE);
                e.bank_second_half |= has(BANK_SECOND_HALF);
                e.no_wear |= has(NO_WEAR);
                e.no_item_loss |= has(NO_ITEM_LOSS);
                // The Red Phoenix Charm opens the second half too (`SetSkillAbility` case 42, `docs/original/item-flags.md` section 5).
                e.bank_second_half |= has(RED_PHOENIX);
                e.exp_up |= has(EXP_UP);
                e.no_exp_loss |= has(NO_EXP_LOSS);
                e.rebirth |= has(REBIRTH);
                if let Some(level) = CHARMS.filter(|&t| has(t)).map(|t| t - 39).max() {
                    e.amulet = e.amulet.max(level);
                }
                if let Some(&(_, v)) = d.abilities.iter().find(|&&(t, v)| t == MOB_GOLD && v != 0) {
                    e.mob_gold = e.mob_gold.max(i32::from(v));
                }
            }
            if d.kind == TRANSFORMATION && matches!(d.type_effect, BUFF | DEBUFF) {
                e.shape = d.change_type.min(u16::from(u8::MAX)) as u8;
                e.shape_mob = a.mob;
                e.rooted |= d.type_effect == DEBUFF && d.change_type == SHAPE_POLYMORPH;
            }
            if d.type_effect == DEBUFF {
                match d.kind {
                    STUN => e.stun = true,
                    STOP => e.stop = true,
                    SLEEP => e.sleep = true,
                    NO_ELEMENT => e.no_element = true,
                    _ => {}
                }
            }
            if d.type_effect != BUFF {
                continue;
            }
            match d.kind {
                HIDE => e.shape = SHAPE_HIDDEN,
                BLOCK_ALL => e.block_all = true,
                GIVE_UP => e.give_up = d.level,
                ATTACK_ELEMENT if d.element < 9 => e.attack_element = Some(d.element),
                DEFENCE_ELEMENT => e.defence_element = Some(d.element),
                _ => {}
            }
        }
        e
    }

    /// `CUser::ChkBlock(kind)` (section 10): block-all, else the newest block buff of that attack kind: a count (used up at 0) or a
    /// chance in percent. `rand` like the C library's.
    pub fn block(&mut self, kind: u8, rand: &mut impl FnMut() -> u32) -> Block {
        if self.list.iter().any(|a| a.is(BUFF, BLOCK_ALL)) {
            return Block { blocked: true, used_up: None };
        }
        let newest = self
            .list
            .iter()
            .rposition(|a| a.def.type_effect == BUFF && matches!(a.def.kind, BLOCK_COUNT | BLOCK_CHANCE) && a.def.defense_type == kind);
        let Some(i) = newest else { return Block::default() };
        let a = &mut self.list[i];
        if a.def.kind == BLOCK_COUNT {
            a.blocks_left -= 1;
            let used_up = (a.blocks_left <= 0).then(|| self.list.remove(i));
            return Block { blocked: true, used_up };
        }
        let chance = i32::from(a.def.defense_value);
        Block { blocked: chance == 100 || (rand() % 100) as i32 <= chance, used_up: None }
    }

    /// `CUser::ChkMirror(kind)` (section 10): whether the newest reflection buff covering that attack kind reflects now, and its
    /// skill id (a monster resisting that skill is not reflected upon). `luc` and `wis` are the player's.
    pub fn reflects(&self, kind: usize, luc: i32, wis: i32, rand: &mut impl FnMut() -> u32) -> Option<u16> {
        let a = self.list.iter().rev().find(|a| a.is(BUFF, MIRROR) && a.def.abilities.get(kind).is_some_and(|&(_, v)| v != 0))?;
        let value = i32::from(a.def.success_value);
        let roll = (rand() % 100) as i32;
        let hit = match a.def.success_type {
            1 => roll <= value,
            2 if value != 0 => roll <= luc * 10 / value,
            3 if value != 0 => roll <= wis * 10 / value,
            _ => false,
        };
        hit.then_some(a.def.id)
    }

    /// `CUser::ChkHPAbilityApply` (section 11), after a hit the player survived: the low-HP passive takes effect when it is not
    /// already in effect, has rested 180 s, and HP is at or below its share of max HP. Returns the new entry (30 s long).
    pub fn low_hp_check(&mut self, hp: u16, max_hp: u16, now: Instant, key: &mut impl FnMut() -> u32) -> Option<Applied> {
        let def = self.low_hp?;
        if self.list.iter().any(|a| a.is(PASSIVE, LOW_HP)) || self.low_hp_last.is_some_and(|last| now < last + LOW_HP_REST) {
            return None;
        }
        let limit = (f32::from(def.limit_hp) * f32::from(max_hp) / 100.0) as i32;
        if i32::from(hp) > limit {
            return None;
        }
        let new = Applied::new(key(), &def, 0, now, now + LOW_HP_DURATION);
        self.list.push(new);
        self.low_hp_last = Some(now);
        Some(new)
    }

    /// The `0x010A` entries: seconds left, at least 1 (`CUser::SendCharacterSetSkill`).
    pub fn burst_entries(&self, now: Instant) -> Vec<AppliedEntry> {
        self.list
            .iter()
            .map(|a| AppliedEntry { key: a.key, skill_id: a.def.id, level: a.def.level, seconds: a.seconds_left(now).max(1) })
            .collect()
    }

    /// What is saved (section 13): buffs and debuffs that are not unsaved transformations, with the seconds left (at most the keep
    /// time).
    pub fn saved(&self, now: Instant) -> Vec<SavedApplied> {
        self.list
            .iter()
            .filter(|a| matches!(a.def.type_effect, BUFF | DEBUFF))
            .filter(|a| !(a.def.kind == TRANSFORMATION && matches!(a.def.change_type, SHAPE_DISGUISE | SHAPE_ILLUSION)))
            .map(|a| SavedApplied { skill_id: a.def.id, level: a.def.level, seconds: a.seconds_left(now).min(a.def.keep_time) })
            .collect()
    }

    /// The list from what was saved (section 13), `offline` seconds after it was saved, on `map`: only buffs and debuffs with a
    /// definition; skills kept through death have counted down offline (and are gone when that ran them out), the others have
    /// not; each gets 10 s more, but never more than its keep time.
    pub fn load(saved: &[SavedApplied], defs: &SkillDefs, offline: u64, map: u16, now: Instant, key: &mut impl FnMut() -> u32) -> Self {
        let mut list = Vec::new();
        for s in saved {
            let Some(def) = defs.get(s.skill_id, s.level) else { continue };
            if !matches!(def.type_effect, BUFF | DEBUFF) || refused_on_map(map, def) {
                continue;
            }
            let seconds = u64::from(s.seconds);
            let left = if def.keep_on_death {
                if seconds < offline {
                    continue;
                }
                seconds - offline + LOAD_GRACE
            } else {
                seconds + LOAD_GRACE
            };
            let ends = now + Duration::from_secs(left.min(u64::from(def.keep_time)));
            list.push(Applied::new(key(), def, 0, now, ends));
        }
        Self { list, ..Self::default() }
    }
}

/// Give up SP/MP (`m_nGiveupSPMP`, `CUser::Damage`, section 10): HP damage paid from SP (mode 1), MP (2), or SP then MP (3) first.
/// `health` is HP, MP, SP; returns the HP damage left and what SP and MP paid, as `0x0605` shows them (HP, SP, MP).
pub fn give_up(mode: u8, damage: u16, health: &mut [u16; 3]) -> [u16; 3] {
    let pay = |left: &mut u16, pool: &mut u16| -> u16 {
        let paid = (*left).min(*pool);
        *pool -= paid;
        *left -= paid;
        paid
    };
    let mut left = damage;
    let (mut sp, mut mp) = (0, 0);
    if matches!(mode, 1 | 3) {
        sp = pay(&mut left, &mut health[2]);
    }
    if matches!(mode, 2 | 3) {
        mp = pay(&mut left, &mut health[1]);
    }
    [left, sp, mp]
}

/// The heal over time a `wType` 81 buff pays for `elapsed` (section 5; every three seconds and at its end, part 4 section 7):
/// `trunc(elapsed / 3000 × wTimeHeal)` of HP, SP and MP.
pub fn time_heal(def: &SkillDef, elapsed: Duration) -> [u16; 3] {
    let ticks = elapsed.as_millis() as f64 / TICK_MS;
    def.time_heal.map(|v| if v == 0 { 0 } else { (ticks * f64::from(v)).min(f64::from(u16::MAX)) as u16 })
}

/// What a field deals to whoever is in it (`TrabDamage`, part 4 section 8), by `byDamageType`: 0 (and others) `wDamage` as it is; 5
/// a percentage of the current HP, SP, MP, 4 of the maxima (single precision, truncated, at least 1 each when not 0); at most
/// 30000. `current` and `max` are HP, SP, MP.
pub fn field_damage(def: &SkillDef, current: [u32; 3], max: [u32; 3]) -> [u32; 3] {
    let share = |i: usize, of: [u32; 3]| match def.damage[i] {
        0 => 0,
        v => ((f32::from(v) / 100.0 * of[i] as f32) as u32).max(1),
    };
    let damage: [u32; 3] = match def.damage_type {
        4 => std::array::from_fn(|i| share(i, max)),
        5 => std::array::from_fn(|i| share(i, current)),
        _ => def.damage.map(u32::from),
    };
    damage.map(|v| v.min(30_000))
}

/// One debuff on a monster (`CMob::m_applySkill`, section 12).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobDebuff {
    pub def: SkillDef,
    /// When it was applied, or its damage over time last dealt (`m_dwPrevTick`).
    pub started: Instant,
    pub ends: Instant,
    /// The player who applied it: the damage over time is theirs.
    pub attacker: u32,
}

/// What applying a debuff to a monster did: entries ended on the way (their end effects follow), whether the list changed or was
/// refreshed, and a taunt to carry out.
#[derive(Debug, Default, PartialEq)]
pub struct MobApplying {
    pub ended: Vec<MobDebuff>,
    pub applied: bool,
    pub taunt: bool,
}

/// `CMob::AddApplySkill` (section 12): debuffs only.
pub fn apply_mob(list: &mut Vec<MobDebuff>, def: &SkillDef, attacker: u32, now: Instant) -> MobApplying {
    if def.type_effect != DEBUFF {
        return MobApplying::default();
    }
    let mut ended = Vec::new();
    let new = MobDebuff { def: *def, started: now, ends: end_of(def, now), attacker };
    let taunt = def.kind == TAUNT && attacker != 0;
    for i in 0..list.len() {
        if list[i].def.id != def.id {
            continue;
        }
        if list[i].def.level == def.level {
            list[i] = MobDebuff { def: list[i].def, ..new };
            return MobApplying { ended, applied: true, taunt };
        }
        if def.level <= 3 {
            if def.level <= list[i].def.level {
                return MobApplying::default();
            }
            ended.push(list.remove(i));
            break;
        }
    }
    list.push(new);
    MobApplying { ended, applied: true, taunt }
}

/// What a monster's debuffs add up to (`CMob::SetSkillAbility` and the state counters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MobEffects {
    pub adds: [Numbers; 3],
    pub wis: i32,
    pub dex: i32,
    pub luc: i32,
    pub attack_speed: i32,
    pub move_speed: i32,
    pub disabled: [i32; 3],
    pub stop: i32,
    pub stun: i32,
    pub sleep: i32,
    /// `wType` 68: no element defence.
    pub no_element: bool,
}

impl MobEffects {
    /// `CMob::GetAttackSpeed`, `GetMoveSpeed`: 255 while stunned or asleep (and stopped, for moving), else the speed clamped.
    pub fn speeds(&self) -> (u8, u8) {
        let held = self.stun > 0 || self.sleep > 0;
        let attack = if held { CANNOT } else { (MOB_ATTACK_SPEED + self.attack_speed).clamp(0, 6) as u8 };
        let movement = if held || self.stop > 0 { CANNOT } else { (MOB_MOVE_SPEED + self.move_speed).clamp(0, 4) as u8 };
        (attack, movement)
    }

    fn add(&mut self, ability: u8, value: i32) {
        match ability {
            7 => self.wis += value,
            8 => self.dex += value,
            9 => self.luc += value,
            16 => self.attack_speed += value,
            17 => self.move_speed += value,
            20..=22 => self.adds[usize::from(ability - 20)].hit += value,
            23..=25 => self.adds[usize::from(ability - 23)].attack += value,
            26..=28 => self.adds[usize::from(ability - 26)].defence += value,
            29..=31 => self.adds[usize::from(ability - 29)].evasion += value,
            32..=34 => self.disabled[usize::from(ability - 32)] += value,
            _ => {}
        }
    }
}

/// A monster's debuffs added up: abilities taken off (62, 67), stun, stop, sleep, no element.
pub fn mob_effects(list: &[MobDebuff]) -> MobEffects {
    let mut e = MobEffects::default();
    for d in list.iter().map(|m| &m.def) {
        match d.kind {
            MOB_ABILITIES | MOB_ABILITIES_TOO => {
                for (ability, value) in d.abilities {
                    e.add(ability, -i32::from(value));
                }
            }
            STUN => e.stun += 1,
            STOP => e.stop += 1,
            SLEEP => e.sleep += 1,
            NO_ELEMENT => e.no_element = true,
            _ => {}
        }
    }
    e
}

/// `CMob::TimeDamage(elapsed)` (section 12): the base by `byTimeDamageType` (0 the values, 3 times the monster's current values,
/// 4 and 5 a percentage of its maximum and current values, at least 1 each), then × elapsed / 3000, truncated, at most 30000.
/// `current` and `max` are HP, SP, MP.
pub fn time_damage(def: &SkillDef, current: [u32; 3], max: [u32; 3], elapsed: Duration) -> [u16; 3] {
    let base: [f64; 3] = std::array::from_fn(|i| {
        let v = def.time_damage[i];
        let at_least_one = |x: f64| if v == 0 { 0.0 } else { x.max(1.0) };
        match def.time_damage_type {
            3 => at_least_one(f64::from(current[i]) * f64::from(v)),
            4 => at_least_one((f64::from(v) / 100.0 * f64::from(max[i])).trunc()),
            5 => at_least_one((f64::from(v) / 100.0 * f64::from(current[i])).trunc()),
            _ => f64::from(v),
        }
    });
    let ticks = elapsed.as_millis() as f64 / TICK_MS;
    base.map(|b| (b * ticks).trunc().min(30_000.0) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buff(id: u16, level: u8, kind: u16, keep: u32) -> SkillDef {
        SkillDef { id, level, type_effect: BUFF, kind, keep_time: keep, ..Default::default() }
    }

    fn keys() -> impl FnMut() -> u32 {
        let mut next = 0;
        move || {
            next += 1;
            next
        }
    }

    #[test]
    fn item_flags_add_up_as_the_original() {
        let now = Instant::now();
        let mut key = keys();
        let item = |id, ability, value| SkillDef { abilities: [(ability, value), (0, 0), (0, 0)], ..buff(id, 1, ABILITIES, 720) };
        let mut b = PlayerBuffs::default();
        let none = b.effects();
        assert_eq!((none.exp_factor(), none.death_exp_percent(), none.gold_percent()), (1.0, 5, 0));
        assert_eq!(none.with_gold_bonus(1000), 1000);
        // An EXP stone: ×2 (deviation 1); the Gold Drop item: its 100 %.
        b.apply(&item(223, 35, 1), 0, now, &mut key);
        b.apply(&item(235, 44, 100), 0, now, &mut key);
        let e = b.effects();
        assert!(e.exp_up && e.exp_factor() == 2.0);
        assert_eq!((e.gold_percent(), e.with_gold_bonus(333)), (100, 666));
        // The White Tiger Charm: + 0.2, 1 % lost at death, + 20 % gold.
        b.apply(&item(232, 41, 1), 0, now, &mut key);
        let e = b.effects();
        assert_eq!((e.amulet, e.exp_factor(), e.death_exp_percent(), e.gold_percent()), (2, 2.2, 1, 120));
        assert!(!e.bank_second_half, "only the Red Phoenix opens the second half");
        // The Red Phoenix Charm: nothing lost at death, + 50 % gold, the warehouse's second half.
        let mut r = PlayerBuffs::default();
        r.apply(&item(233, 42, 1), 0, now, &mut key);
        let e = r.effects();
        assert_eq!((e.amulet, e.exp_factor(), e.death_exp_percent(), e.gold_percent()), (3, 1.2, 0, 50));
        assert!(e.bank_second_half);
        // The Blue Dragon Charm: no gold; Prevent EXP Drop: nothing lost whatever the charm; a Continuous Res Rune.
        let mut d = PlayerBuffs::default();
        d.apply(&item(231, 40, 1), 0, now, &mut key);
        assert_eq!((d.effects().gold_percent(), d.effects().death_exp_percent()), (0, 1));
        d.apply(&item(240, 38, 1), 0, now, &mut key);
        d.apply(&item(266, 46, 1), 0, now, &mut key);
        let e = d.effects();
        assert!(e.no_exp_loss && e.rebirth);
        assert_eq!(e.death_exp_percent(), 0);
        // Prevent Drop Equipped Item (45) sets nothing (section 6).
        let mut q = PlayerBuffs::default();
        q.apply(&item(236, 45, 1), 0, now, &mut key);
        assert_eq!(q.effects(), BuffEffects { bonuses: q.effects().bonuses, ..BuffEffects::default() });
    }

    #[test]
    fn premium_sets_the_billing_flags() {
        let now = Instant::now();
        let mut key = keys();
        let individual = PlayerBuffs { premium: 1, ..PlayerBuffs::default() }.effects();
        assert!(individual.bank_second_half && !individual.no_wear && !individual.exp_premium);
        assert_eq!(individual.exp_factor(), 1.0);
        for kind in [2, 3] {
            let mut b = PlayerBuffs { premium: kind, ..PlayerBuffs::default() };
            let e = b.effects();
            assert!(e.bank_second_half && e.no_wear && e.exp_premium);
            assert_eq!(e.exp_factor(), 1.5, "premium alone");
            let stone = SkillDef { abilities: [(35, 1), (0, 0), (0, 0)], ..buff(223, 1, ABILITIES, 720) };
            b.apply(&stone, 0, now, &mut key);
            assert_eq!(b.effects().exp_factor(), 2.0, "with a stone, as the original");
        }
        assert_eq!(PlayerBuffs { premium: 4, ..PlayerBuffs::default() }.effects(), BuffEffects::default());
    }

    #[test]
    fn same_level_refreshes_higher_replaces_lower_does_nothing() {
        let now = Instant::now();
        let mut key = keys();
        let mut b = PlayerBuffs::default();
        let first = b.apply(&buff(30, 2, ABILITIES, 300), 7, now, &mut key);
        assert_eq!(first, Applying { ended: vec![], set: Some(1), fresh: true });
        let later = now + Duration::from_secs(100);
        let again = b.apply(&buff(30, 2, ABILITIES, 300), 7, later, &mut key);
        assert_eq!(again.set, Some(1), "a refresh keeps the key");
        assert_eq!(b.list[0].ends, later + Duration::from_secs(300));
        assert_eq!(b.apply(&buff(30, 1, ABILITIES, 300), 7, later, &mut key), Applying::default(), "a lower level is ignored");
        let higher = b.apply(&buff(30, 3, ABILITIES, 300), 7, later, &mut key);
        assert_eq!((higher.ended.len(), higher.set), (1, Some(2)));
        assert_eq!((b.list.len(), b.list[0].def.level), (1, 3));
        // Level 100 lives beside levels 1 to 3.
        b.apply(&buff(30, 100, ABILITIES, 10), 0, later, &mut key);
        assert_eq!(b.list.len(), 2);
    }

    #[test]
    fn cures_and_protections() {
        let now = Instant::now();
        let mut key = keys();
        let mut b = PlayerBuffs::default();
        b.apply(&SkillDef { state_type: 3, ..buff(30, 1, ABILITIES, 300) }, 7, now, &mut key);
        let poison = SkillDef { type_effect: DEBUFF, state_type: 9, ..buff(40, 1, DAMAGE_OVER_TIME, 30) };
        b.apply(&poison, 0, now, &mut key);
        b.apply(&SkillDef { type_effect: DEBUFF, ..buff(41, 1, STUN, 5) }, 0, now, &mut key);
        assert_eq!(b.cure(&[9, 10, 11], false).iter().map(|a| a.def.id).collect::<Vec<_>>(), [40]);
        assert_eq!(b.cure(&[], true).iter().map(|a| a.def.id).collect::<Vec<_>>(), [41], "every debuff");
        assert_eq!(b.cure(&[3], false).len(), 1, "a buff of the state goes too");
        assert!(!b.protected(7));
        b.block_dying = true;
        assert!(!b.protected(8));
        assert!(b.protected(7) && !b.protected(7), "used up");
    }

    #[test]
    fn item_skills_refused() {
        let now = Instant::now();
        let mut key = keys();
        let mut b = PlayerBuffs::default();
        let exp_stone = SkillDef { abilities: [(35, 1), (0, 0), (0, 0)], ..buff(222, 1, ABILITIES, 168) };
        assert!(!b.item_skill_refused(&exp_stone));
        b.apply(&exp_stone, 1, now, &mut key);
        let other_stone = SkillDef { id: 250, ..exp_stone };
        assert!(b.item_skill_refused(&other_stone), "an EXP stone is in effect");
        let blue = SkillDef { abilities: [(40, 1), (0, 0), (0, 0)], ..buff(231, 1, ABILITIES, 240) };
        let red = SkillDef { abilities: [(42, 1), (0, 0), (0, 0)], ..buff(233, 1, ABILITIES, 720) };
        b.apply(&red, 1, now, &mut key);
        assert!(b.item_skill_refused(&blue), "a weaker charm");
        assert!(b.item_skill_refused(&red), "the same charm");
        b.apply(&buff(216, 3, ABILITIES, 7200), 1, now, &mut key);
        assert!(b.item_skill_refused(&buff(216, 2, ABILITIES, 7200)), "a higher level is applied");
        assert!(!b.item_skill_refused(&buff(216, 3, ABILITIES, 7200)), "the same level refreshes");
    }

    #[test]
    fn one_attack_element_and_one_exclusive_skill_at_a_time() {
        let now = Instant::now();
        let mut key = keys();
        let mut b = PlayerBuffs::default();
        b.apply(&SkillDef { element: 1, ..buff(316, 1, ATTACK_ELEMENT, 300) }, 7, now, &mut key);
        let water = b.apply(&SkillDef { element: 4, ..buff(319, 1, ATTACK_ELEMENT, 300) }, 7, now, &mut key);
        assert_eq!(water.ended.iter().map(|a| a.def.id).collect::<Vec<_>>(), [316]);
        assert_eq!(b.effects().attack_element, Some(4));
        b.apply(&buff(114, 1, ABILITIES, 60), 7, now, &mut key);
        let hide = b.apply(&buff(63, 1, HIDE, 10), 7, now, &mut key);
        assert_eq!(hide.ended.iter().map(|a| a.def.id).collect::<Vec<_>>(), [114]);
    }

    #[test]
    fn effects_add_up_from_scratch() {
        let now = Instant::now();
        let mut key = keys();
        let mut b = PlayerBuffs::default();
        b.apply(&SkillDef { abilities: [(4, 10), (23, 30), (0, 0)], ..buff(30, 1, ABILITIES, 300) }, 7, now, &mut key);
        b.apply(&SkillDef { abilities: [(16, 2), (0, 0), (0, 0)], ..buff(58, 1, ABILITIES_TOO, 30) }, 7, now, &mut key);
        b.apply(&buff(90, 1, BLOCK_ALL, 9), 7, now, &mut key);
        b.apply(&SkillDef { level: 2, ..buff(74, 2, GIVE_UP, 60) }, 7, now, &mut key);
        let debuff = SkillDef { type_effect: DEBUFF, abilities: [(4, 3), (0, 0), (0, 0)], ..buff(33, 1, MOB_ABILITIES, 6) };
        b.apply(&debuff, 0, now, &mut key);
        let e = b.effects();
        assert_eq!(e.bonuses.stats[0], 10 - 3, "STR up by the buff, down by the debuff");
        assert_eq!((e.bonuses.adds[0].attack, e.bonuses.attack_speed), (30, 2));
        assert!(e.block_all);
        assert_eq!(e.give_up, 2);
        assert_eq!(b.expire(now + Duration::from_secs(7)).len(), 1, "the debuff ran out");
        assert_eq!(b.effects().bonuses.stats[0], 10);
    }

    #[test]
    fn cancel_death_and_dispel() {
        let now = Instant::now();
        let mut key = keys();
        let mut b = PlayerBuffs::default();
        b.apply(&buff(30, 1, ABILITIES, 300), 7, now, &mut key);
        b.apply(&SkillDef { keep_on_death: true, ..buff(250, 1, ABILITIES, 3600) }, 0, now, &mut key);
        let debuff = |id, state_type| SkillDef { type_effect: DEBUFF, state_type, ..buff(id, 1, MOB_ABILITIES, 30) };
        b.apply(&debuff(40, 10), 0, now, &mut key);
        b.apply(&debuff(41, 3), 0, now, &mut key);
        b.apply(&debuff(42, 0), 0, now, &mut key);
        assert_eq!(b.cancel(3), None, "only buffs can be cancelled");
        assert_eq!(b.dispel(1).iter().map(|a| a.def.id).collect::<Vec<_>>(), [40]);
        assert_eq!(b.dispel(2).iter().map(|a| a.def.id).collect::<Vec<_>>(), [41]);
        assert_eq!(b.dispel(3).iter().map(|a| a.def.id).collect::<Vec<_>>(), [42]);
        assert_eq!(b.cancel(1).map(|a| a.def.id), Some(30));
        b.apply(&buff(31, 1, ABILITIES, 300), 7, now, &mut key);
        assert_eq!(b.clear_on_death(false).iter().map(|a| a.def.id).collect::<Vec<_>>(), [31]);
        assert_eq!(b.list.iter().map(|a| a.def.id).collect::<Vec<_>>(), [250], "kept through death");
    }

    #[test]
    fn death_without_experience_loss_keeps_item_buffs() {
        let now = Instant::now();
        let mut key = keys();
        let mut b = PlayerBuffs::default();
        b.apply(&buff(31, 1, ABILITIES, 300), 7, now, &mut key);
        b.apply(&buff(207, 2, ABILITIES, 10800), 7, now, &mut key);
        b.apply(&buff(230, 1, ABILITIES, 3600), 7, now, &mut key);
        b.apply(&buff(9, 100, ABILITIES, 30), 0, now, &mut key);
        b.apply(&SkillDef { keep_on_death: true, ..buff(60, 1, ABILITIES, 3600) }, 7, now, &mut key);
        let gone = b.clear_on_death(true).iter().map(|a| a.def.id).collect::<Vec<_>>();
        assert_eq!(gone, [31, 9], "a class buff and a monster's skill go; the elixir and the potion stay");
        assert_eq!(b.list.iter().map(|a| a.def.id).collect::<Vec<_>>(), [207, 230, 60]);
        assert_eq!(b.clear_on_death(false).iter().map(|a| a.def.id).collect::<Vec<_>>(), [207, 230]);
        assert_eq!(b.list.iter().map(|a| a.def.id).collect::<Vec<_>>(), [60]);
    }

    #[test]
    fn blocks_by_count_and_by_chance() {
        let now = Instant::now();
        let mut key = keys();
        let mut b = PlayerBuffs::default();
        let mut rand = || 50;
        assert_eq!(b.block(2, &mut rand), Block::default());
        b.apply(&SkillDef { defense_type: 2, defense_value: 2, ..buff(54, 1, BLOCK_COUNT, 60) }, 7, now, &mut key);
        assert!(b.block(2, &mut rand).blocked);
        assert!(!b.block(0, &mut rand).blocked, "another attack kind");
        let last = b.block(2, &mut rand);
        assert!(last.blocked && last.used_up.is_some(), "the second block uses it up");
        assert!(b.list.is_empty());
        b.apply(&SkillDef { defense_type: 1, defense_value: 40, ..buff(73, 1, BLOCK_CHANCE, 9) }, 7, now, &mut key);
        assert!(!b.block(1, &mut rand).blocked, "50 is above 40");
        assert!(b.block(1, &mut || 140).blocked, "40 is at most 40");
    }

    #[test]
    fn reflection_follows_its_success_type() {
        let now = Instant::now();
        let mut key = keys();
        let mut b = PlayerBuffs::default();
        let counter = SkillDef { abilities: [(0, 1), (0, 0), (0, 0)], success_type: 1, success_value: 100, ..buff(89, 1, MIRROR, 8) };
        b.apply(&counter, 7, now, &mut key);
        assert_eq!(b.reflects(0, 10, 10, &mut || 99), Some(89));
        assert_eq!(b.reflects(2, 10, 10, &mut || 0), None, "magic is not covered");
    }

    #[test]
    fn the_low_hp_passive_lasts_30_s_and_rests_180_s() {
        let now = Instant::now();
        let mut key = keys();
        let def = SkillDef { id: 28, level: 1, kind: LOW_HP, limit_hp: 10, abilities: [(19, 500), (0, 0), (0, 0)], ..Default::default() };
        let mut b = PlayerBuffs { low_hp: Some(def), ..Default::default() };
        assert_eq!(b.low_hp_check(11, 100, now, &mut key), None, "above 10 %");
        let on = b.low_hp_check(10, 100, now, &mut key).unwrap();
        assert_eq!(on.ends, now + LOW_HP_DURATION);
        assert_eq!(b.effects().bonuses.cool_time, 500);
        assert_eq!(b.low_hp_check(1, 100, now, &mut key), None, "already in effect");
        b.expire(now + LOW_HP_DURATION);
        assert_eq!(b.low_hp_check(1, 100, now + Duration::from_secs(179), &mut key), None, "resting");
        assert!(b.low_hp_check(1, 100, now + Duration::from_secs(180), &mut key).is_some());
    }

    #[test]
    fn saving_and_loading_keep_the_time_left() {
        let now = Instant::now();
        let mut key = keys();
        let normal = buff(30, 1, ABILITIES, 300);
        let item = SkillDef { keep_on_death: true, ..buff(250, 1, ABILITIES, 3600) };
        let transformation = SkillDef { change_type: 100, ..buff(77, 1, TRANSFORMATION, 60) };
        let mut b = PlayerBuffs::default();
        for d in [normal, item, transformation] {
            b.apply(&d, 7, now, &mut key);
        }
        let saved = b.saved(now + Duration::from_secs(100));
        assert_eq!(saved, [SavedApplied { skill_id: 30, level: 1, seconds: 200 }, SavedApplied { skill_id: 250, level: 1, seconds: 3500 }]);
        let defs = SkillDefs::new([normal, SkillDef { keep_time: 1, ..item }]);
        // An hour offline: the normal buff stood still, the item buff (keep time corrected to 3600 s) ran on and ran out.
        let loaded = PlayerBuffs::load(&saved, &defs, 3600, 1, now, &mut key);
        assert_eq!(loaded.list.len(), 1);
        assert_eq!(loaded.list[0].ends, now + Duration::from_secs(210), "10 s more");
        let soon = PlayerBuffs::load(&saved, &defs, 100, 1, now, &mut key);
        assert_eq!(soon.list[1].ends, now + Duration::from_secs(3410));
        let capped = PlayerBuffs::load(&[SavedApplied { skill_id: 30, level: 1, seconds: 295 }], &defs, 0, 1, now, &mut key);
        assert_eq!(capped.list[0].ends, now + Duration::from_secs(300), "never above the keep time");
        assert_eq!(capped.burst_entries(now)[0].seconds, 300);
    }

    #[test]
    fn shapes_and_what_ends_them() {
        let now = Instant::now();
        let mut key = keys();
        let mut b = PlayerBuffs::default();
        let hide = buff(63, 1, HIDE, 10);
        b.apply(&hide, 7, now, &mut key);
        assert_eq!((b.effects().shape, b.effects().shape_mob), (SHAPE_HIDDEN, 0));
        assert_eq!(b.reveal().map(|a| a.def.id), Some(63));
        assert_eq!(b.effects().shape, 0);
        // An illusion's monster is the copied one; Evolution's its `wChangeLevel`.
        let illusion = SkillDef { change_type: SHAPE_ILLUSION, ..buff(77, 1, TRANSFORMATION, 60) };
        let set = b.apply(&illusion, 7, now, &mut key).set.unwrap();
        b.set_mob(set, 413);
        assert_eq!((b.effects().shape, b.effects().shape_mob), (102, 413));
        assert_eq!(b.end_shape_for_attack().map(|a| a.def.id), Some(77), "fighting ends an illusion");
        let evolution = SkillDef { change_type: 10, change_level: 2049, ..buff(127, 1, TRANSFORMATION, 60) };
        b.apply(&evolution, 7, now, &mut key);
        assert_eq!((b.effects().shape, b.effects().shape_mob), (10, 2049));
        assert_eq!(b.end_shape_for_attack(), None, "but not Evolution");
        assert_eq!(b.reveal(), None, "nor does being found");
        // Disguises and illusions are not saved (the copied looks and monster are not kept).
        let disguise = SkillDef { change_type: SHAPE_DISGUISE, ..buff(107, 1, TRANSFORMATION, 180) };
        for d in [disguise, illusion] {
            let mut b = PlayerBuffs::default();
            b.apply(&d, 7, now, &mut key);
            assert!(b.saved(now).is_empty());
        }
        assert_eq!(b.saved(now).len(), 1, "Evolution is");
    }

    #[test]
    fn what_fields_deal() {
        let trap = SkillDef { damage_type: 5, damage: [15, 0, 3], ..Default::default() };
        assert_eq!(field_damage(&trap, [968, 0, 10], [1000, 50, 50]), [145, 0, 1]);
        assert_eq!(field_damage(&SkillDef { damage_type: 4, ..trap }, [968, 0, 10], [1000, 50, 50]), [150, 0, 1]);
        let wall = SkillDef { damage: [32, 0, 0], ..Default::default() };
        assert_eq!(field_damage(&wall, [1, 0, 0], [1, 0, 0]), [32, 0, 0]);
        assert_eq!(field_damage(&SkillDef { damage: [40_000, 0, 0], ..wall }, [0; 3], [0; 3]), [30_000, 0, 0]);
    }

    #[test]
    fn giving_up_sp_and_mp() {
        let mut h = [100, 30, 50];
        assert_eq!(give_up(1, 40, &mut h), [0, 40, 0]);
        assert_eq!(h, [100, 30, 10]);
        assert_eq!(give_up(3, 25, &mut h), [0, 10, 15], "SP runs out, MP pays the rest");
        assert_eq!(h, [100, 15, 0]);
        assert_eq!(give_up(2, 40, &mut h), [25, 0, 15]);
        assert_eq!(give_up(0, 40, &mut h), [40, 0, 0]);
    }

    #[test]
    fn the_map_rule() {
        assert!(refused_on_map(18, &buff(124, 1, ABILITIES, 1)));
        assert!(refused_on_map(18, &buff(111, 2, ABILITIES, 1)));
        assert!(!refused_on_map(18, &buff(111, 1, ABILITIES, 1)));
        assert!(!refused_on_map(1, &buff(124, 1, ABILITIES, 1)));
    }

    #[test]
    fn heal_over_time_counts_threes_of_seconds() {
        let def = SkillDef { time_heal: [40, 0, 10], ..buff(124, 1, HEAL_OVER_TIME, 18) };
        assert_eq!(time_heal(&def, Duration::from_millis(18_000)), [240, 0, 60]);
        assert_eq!(time_heal(&def, Duration::from_millis(4_500)), [60, 0, 15]);
    }

    #[test]
    fn monster_debuffs() {
        let now = Instant::now();
        let debuff = |id, level, kind| SkillDef { id, level, type_effect: DEBUFF, kind, keep_time: 10, ..Default::default() };
        let mut list = Vec::new();
        assert!(apply_mob(&mut list, &buff(30, 1, ABILITIES, 10), 7, now).ended.is_empty(), "buffs do not apply");
        assert!(list.is_empty());
        let slow = SkillDef { abilities: [(17, 2), (16, 1), (26, 30)], ..debuff(33, 1, MOB_ABILITIES) };
        assert!(apply_mob(&mut list, &slow, 7, now).applied);
        apply_mob(&mut list, &debuff(41, 1, STUN), 7, now);
        let e = mob_effects(&list);
        assert_eq!((e.move_speed, e.attack_speed, e.adds[0].defence), (-2, -1, -30));
        assert_eq!(e.speeds(), (CANNOT, CANNOT), "stunned");
        list.retain(|d| d.def.kind != STUN);
        assert_eq!(mob_effects(&list).speeds(), (2, 0));
        apply_mob(&mut list, &debuff(35, 1, STOP), 7, now);
        assert_eq!(mob_effects(&list).speeds(), (2, CANNOT));
        // A higher level ends the old one (its end effect follows), a lower one is ignored.
        let higher = apply_mob(&mut list, &SkillDef { level: 2, ..slow }, 7, now);
        assert_eq!(higher.ended.len(), 1);
        assert!(!apply_mob(&mut list, &slow, 7, now).applied);
        assert!(apply_mob(&mut list, &debuff(305, 1, TAUNT), 7, now).taunt);
    }

    #[test]
    fn damage_over_time_is_per_three_seconds() {
        let def = SkillDef { time_damage: [159, 0, 0], ..Default::default() };
        assert_eq!(time_damage(&def, [5000, 0, 0], [9000, 0, 0], Duration::from_secs(18)), [954, 0, 0]);
        let share = SkillDef { time_damage_type: 4, time_damage: [1, 0, 0], ..def };
        assert_eq!(time_damage(&share, [5000, 0, 0], [9000, 0, 0], Duration::from_secs(3)), [90, 0, 0]);
        let tiny = SkillDef { time_damage_type: 5, time_damage: [1, 0, 0], ..def };
        assert_eq!(time_damage(&tiny, [50, 0, 0], [9000, 0, 0], Duration::from_secs(6)), [2, 0, 0], "at least 1 a tick");
    }

    #[test]
    fn monster_debuffs_hold_and_sleep_ends_on_waking() {
        let debuff = |id: u16, kind: u16, change_type: u16| SkillDef {
            id,
            level: 100,
            type_effect: DEBUFF,
            kind,
            keep_time: 10,
            change_type,
            ..Default::default()
        };
        let now = Instant::now();
        let mut key = 0;
        let mut next = || {
            key += 1;
            key
        };
        let mut b = PlayerBuffs::default();
        b.apply(&debuff(1, STOP, 0), 9, now, &mut next);
        assert_eq!(b.effects().speeds((5, 2)), (5, CANNOT), "stopped: attacks still go");
        b.apply(&debuff(2, SLEEP, 0), 9, now, &mut next);
        assert_eq!(b.effects().speeds((5, 2)), (CANNOT, CANNOT), "asleep");
        assert_eq!(b.wake().len(), 1);
        assert_eq!(b.effects().speeds((5, 2)), (5, CANNOT));
        let mut b = PlayerBuffs::default();
        b.apply(&debuff(3, TRANSFORMATION, u16::from(SHAPE_POLYMORPH as u8)), 9, now, &mut next);
        let e = b.effects();
        assert_eq!((e.shape, e.rooted, e.speeds((5, 2))), (101, true, (CANNOT, 0)), "a polymorph roots");
        b.apply(&debuff(4, NO_ELEMENT, 0), 9, now, &mut next);
        assert!(b.effects().no_element);
    }
}
