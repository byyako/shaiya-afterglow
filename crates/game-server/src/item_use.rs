//! Using items (`docs/original/item-use.md`): who may use one, the reuse groups, what each `bySpecial` does, the resets and the
//! moves items make. Pure functions; the connection and the zone carry them out.

use std::time::{Duration, Instant};

use crate::inventory::{ItemDef, race_fits, real_type};

/// Item types 25, 44 and 100: the only items `CUser::ItemUse` uses (section 2).
const REAL_TYPE_USABLE: u8 = 26;
/// Quest items (types 27 to 29); type 29 is used up as well.
const REAL_TYPE_QUEST: u8 = 27;
const QUEST_ITEM_USED_UP: u8 = 29;
/// How long a used quest item can start a quest (`m_dwQuestLastUseItemTime`).
pub const QUEST_ITEM_MEMORY: Duration = Duration::from_secs(300);
/// The casting before a move by item (section 9).
pub const MOVE_CAST: Duration = Duration::from_secs(5);
/// How long a summoned member has to answer (`m_dwTimeOverPartyCall`).
pub const PARTY_CALL_WAIT: Duration = Duration::from_secs(35);
/// The result byte of `0x0222` for each move (`m_nSavePosUseIndex`).
pub const RESULT_RECALL: u8 = 255;
pub const RESULT_MOVEMENT_RUNE: u8 = 101;
pub const RESULT_SUMMON: u8 = 100;
/// How long after a skill request (`0x0511`, `0x0517`) a move by item is refused: the skill-cut fix (deviation, section 11).
pub const SKILL_CUT_GUARD: Duration = Duration::from_secs(3);
/// The map the summon stone and the movement rune do not work with.
pub const MAP_NO_PARTY_MOVE: u16 = 67;

/// `g_pPotionReuseDelay`, by reuse group (`byIG`, clamped to 11).
const REUSE_DELAYS_MS: [u64; 12] = [0, 15000, 20000, 25000, 30000, 60000, 120000, 0, 0, 0, 600000, 2000];
const LAST_GROUP: u8 = 11;

/// What using an item does (section 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    /// A quest item: remembered for start-by-use quests; used up for type 29. No answer.
    QuestItem {
        used_up: bool,
    },
    /// Not an item to use, or an effect the original refuses: the error answer.
    Refused,
    /// An effect not built yet: the error answer, the item kept.
    NotBuilt,
    /// The item's skill (`byRange`, `byAttackTime`): a buff on oneself.
    Skill {
        id: u16,
        level: u8,
    },
    /// An experience wheel (special 50): the buff, and the experience back to the level's start (`docs/original/item-windows.md`
    /// section 3).
    ExpWheel {
        id: u16,
        level: u8,
    },
    /// HP, SP and MP added (special 0).
    Recover([u16; 3]),
    /// HP, SP and MP added in percent of the maxima (special 17).
    RecoverPercent([u16; 3]),
    Cure(Cure),
    SkillReset,
    StatReset,
    /// A recall rune (47 to 49): to a fixed place of the player's country after the casting.
    Recall(u8),
    /// The Guild House Teleport Stone (special 56, `docs/original/guild-house.md` section 7).
    GuildHouseStone,
    /// The Teleportation Stone (39): to a saved position after the casting (`docs/original/item-windows.md` section 1).
    Teleport,
    MovementRune,
    PartySummon,
    RepairAll,
    TargetResurrection,
    /// Honor added (special 43).
    Fame(i16),
    Firecracker,
    /// A box or pocket (special 42): an item from set `reqvg` of `ItemCreate.ini` (`docs/original/item-origins.md` section 5).
    Box(i16),
    /// The Goddess Blessing (special 30): the faction's kill count five grades up (`docs/original/pvp-2.md` section 9).
    Blessing,
    /// Message To Server (special 38): the next normal chat line goes to every player (`docs/original/small-items.md` section 5).
    MessageToServer,
    /// The Name Change Stone (special 51): one more rename at the character screen (`docs/original/small-items.md` section 9).
    NameChange,
}

/// What a cure removes and the protections it sets (`CUser::ItemUseRemState`, section 5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cure {
    /// Applied skills of these `byStateType` go.
    pub states: &'static [u8],
    /// Every debuff goes (special 26).
    pub all_debuffs: bool,
    /// `m_bBlockDying` (state 7) and `m_bBlockDeath` (state 8).
    pub block_dying: bool,
    pub block_death: bool,
}

impl Effect {
    /// What the core's own effect is, in words, for `ext check`'s warning that an item with an extension's effect does this while the
    /// extension is disabled (`docs/plans/ext-3-effects.md` section 3.3); `None` for the effects that are a refusal of the item.
    pub fn label(&self) -> Option<&'static str> {
        Some(match self {
            Effect::QuestItem { .. } | Effect::Refused | Effect::NotBuilt => return None,
            Effect::Skill { .. } => "an item that gives a skill",
            Effect::ExpWheel { .. } => "an experience wheel",
            Effect::Recover(_) | Effect::RecoverPercent(_) => "a potion",
            Effect::Cure(_) => "a cure",
            Effect::SkillReset => "a skill reset",
            Effect::StatReset => "a stat reset",
            Effect::Recall(_) | Effect::GuildHouseStone | Effect::Teleport | Effect::MovementRune | Effect::PartySummon => "a move by item",
            Effect::RepairAll => "a repair of everything equipped",
            Effect::TargetResurrection => "a resurrection",
            Effect::Fame(_) => "an honor item",
            Effect::Firecracker => "a firecracker",
            Effect::Box(_) => "a box",
            Effect::Blessing => "a goddess blessing",
            Effect::MessageToServer => "a message to the server",
            Effect::NameChange => "a name change stone",
        })
    }

    /// The effects that start a move by item after a casting (section 9).
    pub fn casts(&self) -> bool {
        matches!(self, Effect::Recall(_) | Effect::Teleport | Effect::MovementRune | Effect::PartySummon)
    }
}

/// The cure of a special, if it is one.
pub fn cure(special: u8) -> Option<Cure> {
    const STATES: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    let only = |states| Some(Cure { states, ..Cure::default() });
    match special {
        7 => Some(Cure { states: &STATES[7..8], block_dying: true, ..Cure::default() }),
        8 => Some(Cure { states: &STATES[8..9], block_death: true, ..Cure::default() }),
        1..=15 => only(&STATES[usize::from(special)..=usize::from(special)]),
        18 => only(&[9, 10, 11]),
        19 => only(&[1, 2, 5, 6]),
        20 => only(&[3, 4]),
        21 => only(&[14, 15]),
        22 => only(&[12, 13]),
        23 => Some(Cure { states: &[7, 8], all_debuffs: false, block_dying: true, block_death: true }),
        26 => Some(Cure { states: &[], all_debuffs: true, block_dying: true, block_death: true }),
        _ => None,
    }
}

/// What an item does when used (section 4), before any check of the user.
pub fn effect(def: &ItemDef) -> Effect {
    match real_type(def.item_type) {
        REAL_TYPE_QUEST => return Effect::QuestItem { used_up: def.item_type == QUEST_ITEM_USED_UP },
        REAL_TYPE_USABLE => {}
        _ => return Effect::Refused,
    }
    if def.range != 0 && def.attack_time != 0 {
        return match def.special {
            // The experience wheels reset the experience as well (section 7).
            50 => Effect::ExpWheel { id: def.range, level: def.attack_time },
            _ => Effect::Skill { id: def.range, level: def.attack_time },
        };
    }
    if let Some(cure) = cure(def.special) {
        return Effect::Cure(cure);
    }
    // Item definitions keep HP, MP, SP; the effects take HP, SP, MP.
    let [hp, mp, sp] = def.const_health;
    match def.special {
        0 if [hp, sp, mp] == [0; 3] => Effect::Refused,
        0 => Effect::Recover([hp, sp, mp]),
        17 => Effect::RecoverPercent([hp, sp, mp]),
        31 => Effect::PartySummon,
        32 => Effect::MovementRune,
        33 => Effect::SkillReset,
        34 => Effect::StatReset,
        40 => Effect::RepairAll,
        41 => Effect::TargetResurrection,
        43 => Effect::Fame(def.vg),
        42 => Effect::Box(def.vg),
        46 => Effect::Firecracker,
        47..=49 => Effect::Recall(def.special),
        39 => Effect::Teleport,
        30 => Effect::Blessing,
        56 => Effect::GuildHouseStone,
        38 => Effect::MessageToServer,
        // No item has special 25 (`docs/original/small-items.md` section 10).
        25 => Effect::Refused,
        51 => Effect::NameChange,
        59 => Effect::NotBuilt,
        _ => Effect::Refused,
    }
}

/// Whether the user's level, race and mode allow the item (section 2 step 4; classes and sex are not checked).
pub fn may_use(def: &ItemDef, level: u16, family: u8, grow: u8) -> bool {
    def.level <= level && race_fits(def.country, family) && def.grow <= grow
}

/// `percent` % of `max`, truncated (special 17). The original multiplies by `0.01f`, a little below 0.01, and loses one on round
/// products (30 % of 1000 is 299 there); this is exact (deviation, `docs/original/item-use.md` section 11).
pub fn percent_of(percent: u16, max: u16) -> u16 {
    (u32::from(percent) * u32::from(max) / 100).min(u32::from(u16::MAX)) as u16
}

/// The time each reuse group is ready again (`m_dwNextPotionReuseTime`); kept in memory only, as the original.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReuseTimes([Option<Instant>; 12]);

/// A reuse group's time taken by a use, to put back if the effect fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Armed {
    group: u8,
    before: Option<Instant>,
}

impl ReuseTimes {
    /// Section 2 step 5: group 0 has no time; another group must be ready, and is then not ready until its delay has passed.
    /// `None` when it is not ready.
    pub fn start(&mut self, group: u8, now: Instant) -> Option<Armed> {
        let group = group.min(LAST_GROUP);
        let slot = &mut self.0[usize::from(group)];
        let before = *slot;
        if group != 0 {
            if before.is_some_and(|ready| now < ready) {
                return None;
            }
            *slot = Some(now + Duration::from_millis(REUSE_DELAYS_MS[usize::from(group)]));
        }
        Some(Armed { group, before })
    }

    /// The effect failed: the group's time as it was.
    pub fn restore(&mut self, armed: Armed) {
        if armed.group != 0 {
            self.0[usize::from(armed.group)] = armed.before;
        }
    }

    /// A move by item cancelled or failed (`CUser::CancleMove`): the group is ready at once (group 11 is left as it is).
    pub fn clear(&mut self, armed: Armed) {
        if armed.group < LAST_GROUP {
            self.0[usize::from(armed.group)] = None;
        }
    }
}

/// `g_ItemSpecialMoveCity`, `Secret`, `Arena` by country (0 Light, 1 Fury): map and position.
pub fn recall_destination(special: u8, country: u8) -> Option<(u16, [f32; 3])> {
    let light = country == 0;
    Some(match special {
        47 if light => (35, [585.87, 27.0, 224.49]),
        47 => (36, [549.1, 42.0, 325.43]),
        48 if light => (42, [78.61, 15.36, 27.95]),
        48 => (42, [23.39, 15.36, 106.4]),
        49 if light => (40, [128.0, 3.5, 82.0]),
        49 => (40, [58.0, 3.5, 82.0]),
        _ => return None,
    })
}

/// Where the Guild House Teleport Stone goes (`g_ItemSpecialMoveGuildHouse`): Light map 51, Fury map 52.
pub fn guild_house_destination(country: u8) -> (u16, [f32; 3]) {
    if country == 0 { (51, [491.49, 41.84, 326.29]) } else { (52, [482.37, 41.84, 321.91]) }
}

/// The level caps of moves by item (`ChkMoveMap`): maps 6 and 8 up to level 35, map 18 up to 15, maps 30 to 32 up to 30.
pub fn level_allows(map: u16, level: u16) -> bool {
    match crate::world::map_of(map) {
        6 | 8 => level <= 35,
        18 => level <= 15,
        30..=32 => level <= 30,
        _ => true,
    }
}

/// A stat reset (section 8): the stat points and base stats (STR, DEX, REC, INT, WIS, LUC) for a character of this class, race, mode
/// and level: the starting stats, one point a level to the class stat, and the points of every level.
pub fn reset_stats(job: u8, family: u8, grow: u8, level: u16) -> (u16, [u16; 6]) {
    let n = level.saturating_sub(1);
    let mut stats = crate::creation::starting_stats(job, family);
    if let Some(stat) = crate::combat::class_stat(job) {
        stats[stat] += n;
    }
    let per_level = match grow {
        2 => 7,
        3 => 9,
        _ => 5,
    };
    (n * per_level, stats)
}

/// A skill reset (section 8): the skill points for a character of this mode and level.
pub fn reset_skill_points(grow: u8, level: u16) -> u16 {
    let n = level.saturating_sub(1);
    let per_level = match grow {
        2 => 4,
        3 => 5,
        _ => 3,
    };
    n * per_level + 5
}

/// The maps where the experience wheels work (`m_nMapID` 18 or 30, section 7).
pub const EXP_WHEEL_MAPS: [u16; 2] = [18, 30];

/// Whether an experience wheel may reset `exp` (section 7): at least halfway between the level's start and end totals (added as
/// `u32`, halved).
pub fn wheel_allows(exp: u32, start: u32, end: u32) -> bool {
    exp >= start.wrapping_add(end) >> 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(item_type: u8, special: u8) -> ItemDef {
        ItemDef { item_type, type_id: 1, special, country: 6, ..Default::default() }
    }

    #[test]
    fn effects_by_special() {
        assert_eq!(effect(&item(27, 0)), Effect::QuestItem { used_up: false });
        assert_eq!(effect(&item(29, 0)), Effect::QuestItem { used_up: true });
        assert_eq!(effect(&item(16, 0)), Effect::Refused, "armour");
        assert_eq!(effect(&item(25, 0)), Effect::Refused, "nothing to add");
        // Item rows keep HP, MP, SP.
        let apple = ItemDef { const_health: [50, 0, 10], ..item(25, 0) };
        assert_eq!(effect(&apple), Effect::Recover([50, 10, 0]));
        let potion = ItemDef { const_health: [30, 0, 0], ..item(25, 17) };
        assert_eq!(effect(&potion), Effect::RecoverPercent([30, 0, 0]));
        let remedy = ItemDef { range: 202, attack_time: 1, ..item(100, 0) };
        assert_eq!(effect(&remedy), Effect::Skill { id: 202, level: 1 });
        let wheel = ItemDef { range: 273, attack_time: 1, ..item(44, 50) };
        assert_eq!(effect(&wheel), Effect::ExpWheel { id: 273, level: 1 });
        assert_eq!(effect(&item(100, 39)), Effect::Teleport);
        assert!(Effect::Teleport.casts());
        // Halfway between 100 and 300 is 200.
        assert!(!wheel_allows(199, 100, 300));
        assert!(wheel_allows(200, 100, 300));
        assert_eq!(effect(&item(100, 33)), Effect::SkillReset);
        assert_eq!(effect(&item(100, 34)), Effect::StatReset);
        assert_eq!(effect(&item(100, 48)), Effect::Recall(48));
        assert_eq!(effect(&ItemDef { vg: 100, ..item(100, 43) }), Effect::Fame(100));
        assert_eq!(effect(&item(100, 36)), Effect::Refused, "a linking hammer is used elsewhere");
        assert_eq!(effect(&item(100, 42)), Effect::Box(0));
        assert_eq!(effect(&item(100, 51)), Effect::NameChange);
        assert_eq!(effect(&item(100, 59)), Effect::NotBuilt);
        assert_eq!(effect(&item(100, 38)), Effect::MessageToServer);
        assert_eq!(effect(&item(100, 25)), Effect::Refused, "no item has special 25");
        assert_eq!(effect(&item(100, 30)), Effect::Blessing);
    }

    #[test]
    fn effects_have_a_word_unless_they_refuse() {
        assert_eq!(effect(&item(100, 0)).label(), None, "special 0 with no HP, SP or MP is a refusal");
        assert_eq!(effect(&ItemDef { const_health: [30, 0, 0], ..item(25, 0) }).label(), Some("a potion"));
        assert_eq!(effect(&item(100, 33)).label(), Some("a skill reset"));
        assert_eq!(effect(&item(100, 59)).label(), None, "not built is an error answer too");
        assert_eq!(effect(&item(27, 0)).label(), None);
    }

    #[test]
    fn cures() {
        assert_eq!(cure(3).unwrap().states, [3]);
        assert_eq!(cure(15).unwrap().states, [15]);
        let faint = cure(7).unwrap();
        assert_eq!((faint.states, faint.block_dying, faint.block_death), (&[7][..], true, false));
        assert_eq!(cure(19).unwrap().states, [1, 2, 5, 6]);
        let perfect = cure(26).unwrap();
        assert!(perfect.all_debuffs && perfect.block_dying && perfect.block_death);
        assert_eq!(cure(0), None);
        assert_eq!(cure(16), None);
        assert_eq!(cure(17), None);
    }

    #[test]
    fn who_may_use() {
        let def = ItemDef { level: 10, country: 4, grow: 2, ..item(100, 0) };
        assert!(may_use(&def, 10, 2, 2));
        assert!(!may_use(&def, 9, 2, 2), "level");
        assert!(!may_use(&def, 10, 3, 2), "vail only");
        assert!(!may_use(&def, 10, 2, 1), "hard items");
        assert!(may_use(&def, 10, 2, 3));
    }

    #[test]
    fn percentages_truncate() {
        // Exact, unlike the original's 0.01f (299 for 30 % of 1000).
        assert_eq!(percent_of(30, 1000), 300);
        assert_eq!(percent_of(30, 333), 99, "99.9");
        assert_eq!(percent_of(30, 1001), 300, "300.3");
        assert_eq!(percent_of(100, 65535), 65535);
        assert_eq!(percent_of(0, 500), 0);
    }

    #[test]
    fn reuse_groups() {
        let now = Instant::now();
        let mut times = ReuseTimes::default();
        let armed = times.start(1, now).unwrap();
        assert_eq!(times.start(1, now + Duration::from_millis(14999)), None);
        assert!(times.start(1, now + Duration::from_millis(15000)).is_some());
        times.restore(armed);
        assert!(times.start(1, now).is_some(), "put back to ready");
        assert!(times.start(0, now).is_some() && times.start(0, now).is_some(), "group 0 has no time");
        // Above 11 counts as 11 (2 s).
        times.start(99, now).unwrap();
        assert_eq!(times.start(11, now + Duration::from_millis(1999)), None);
        let cast = times.start(2, now).unwrap();
        times.clear(cast);
        assert!(times.start(2, now).is_some(), "a cancelled move frees the group");
    }

    #[test]
    fn destinations_and_caps() {
        assert_eq!(recall_destination(47, 1), Some((36, [549.1, 42.0, 325.43])));
        assert_eq!(recall_destination(49, 0), Some((40, [128.0, 3.5, 82.0])));
        assert_eq!(recall_destination(50, 0), None);
        assert!(level_allows(18, 15) && !level_allows(18, 16));
        assert!(level_allows(8, 35) && !level_allows(6, 36));
        assert!(!level_allows(31, 31) && level_allows(33, 60));
    }

    #[test]
    fn resets() {
        // Human fighter, normal, level 10: STR 14 + 9, LUC 11 + 4; 45 points.
        assert_eq!(reset_stats(0, 0, 1, 10), (45, [23, 12, 9, 8, 7, 15]));
        // Deatheater defender, hard, level 1: nothing gained yet.
        assert_eq!(reset_stats(1, 3, 2, 1), (0, [12, 9, 14, 10, 10, 10]));
        // Vail priest, ultimate, level 60: WIS 14 + 2 + 59, 531 points.
        assert_eq!(reset_stats(5, 2, 3, 60), (531, [8, 9, 10, 14, 75, 8]));
        assert_eq!(reset_skill_points(1, 10), 32);
        assert_eq!(reset_skill_points(2, 10), 41);
        assert_eq!(reset_skill_points(3, 1), 5);
    }
}
