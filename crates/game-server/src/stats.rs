//! A character's numbers from its class, level, mode, race, base stats, equipment and passive skills (`docs/original/stats.md`,
//! `docs/original/skills.md` section 5): max HP, SP and MP, the stat totals and the speeds. Attack and defence are in `crate::combat`.

use std::collections::HashMap;

use crate::backend::ItemUnit;
use crate::inventory::{EQUIPMENT_SLOTS, ItemDefs, real_type};
use crate::skills::Passives;

/// Equipment slots whose items count: 0 to 12, not the mount (`CUser::InitEquipment`).
const COUNTED_SLOTS: u8 = EQUIPMENT_SLOTS - 1;
const WEAPON_SLOT: u8 = 5;
const ACCESSORY_SLOTS: std::ops::RangeInclusive<u8> = 8..=12;
/// Move speed before equipment (`CUser::InitEquipment`).
pub const BASE_MOVE_SPEED: i32 = 2;
/// Highest speeds the client is told (`CUser::ItemEquipmentAdd`).
const MAX_ATTACK_SPEED: i32 = 9;
const MAX_MOVE_SPEED: i32 = 4;
const STAT_HEALTH: i32 = 5;
const CRAFT_HEALTH: i32 = 100;
/// A lapis of this special adds recovery, not health.
const RECOVERY_LAPIS: u8 = 16;

/// The status table (`defs.status_defs`): HP, SP and MP by class (0 to 5) and level (1 to 60).
#[derive(Debug, Default)]
pub struct StatusTable(HashMap<(u8, u16), [u16; 3]>);

impl StatusTable {
    /// Rows of `(job, level, hp, sp, mp)`.
    pub fn new(rows: impl IntoIterator<Item = (u8, u16, [u16; 3])>) -> Self {
        Self(rows.into_iter().map(|(job, level, values)| ((job, level), values)).collect())
    }

    /// `[hp, sp, mp]`; outside the table the original uses the row of class 0 level 60 (`CGameData::GetStatusHP`).
    fn row(&self, job: u8, level: u16) -> [u16; 3] {
        self.0.get(&(job, level)).or_else(|| self.0.get(&(0, 60))).copied().unwrap_or_default()
    }
}

/// What the numbers depend on about the character.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Base {
    pub job: u8,
    pub level: u16,
    /// 0 easy, 1 normal, 2 hard, 3 ultimate.
    pub grow: u8,
    /// 0 human, 1 elf, 2 vail, 3 deatheater.
    pub family: u8,
    /// STR, DEX, REC, INT, WIS, LUC as stored.
    pub stats: [u16; 6],
    /// What the faction's kill-count grade gives (`kc`, `docs/original/pvp-2.md` sections 8 and 10).
    pub grade: [i32; crate::kc::TYPES],
}

/// A character's numbers. Stats are STR, DEX, REC, INT, WIS, LUC; health is HP, MP, SP (the order of `0x0105`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub stats: [i32; 6],
    pub max_health: [i32; 3],
    pub attack_speed: i32,
    pub move_speed: i32,
    /// The faction grade's recovery, HP, MP, SP: added to the normal and sitting recovery (`m_nRecHP`, `m_nSitRecHP`, …), and
    /// while fighting (`m_nAtkRecHP`, …).
    pub grade_recovery: [i32; 3],
    pub grade_fight_recovery: [i32; 3],
}

const DEX: usize = 1;
const REC: usize = 2;
const WIS: usize = 4;
const HP: usize = 0;
const MP: usize = 1;
const SP: usize = 2;

impl Totals {
    /// The maxima as `0x0105` and `0x050B` carry them.
    pub fn max_health(&self) -> [u16; 3] {
        self.max_health.map(|v| v.clamp(0, i32::from(u16::MAX)) as u16)
    }

    /// `(attack, move)` as `0x051C` carries them.
    pub fn speeds(&self) -> (u8, u8) {
        (self.attack_speed.clamp(0, MAX_ATTACK_SPEED) as u8, self.move_speed.clamp(0, MAX_MOVE_SPEED) as u8)
    }
}

/// The mode multiplier as the original applies it: the float `1.1f` or `1.5f`, then truncated.
pub fn scaled(value: u16, grow: u8) -> i32 {
    let factor = match grow {
        2 => 1.1f32,
        3 => 1.5f32,
        _ => return i32::from(value),
    };
    (f64::from(value) * f64::from(factor)) as i32
}

/// The outcome of a `0x0208` request (`docs/original/stats.md` section 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spend {
    /// Nothing asked for: no answer.
    Nothing,
    /// More than the stat points left (or a stat past the database's `smallint`): answered with zeros.
    Refused,
    /// The stat points left and the new base stats.
    Spent { stat_point: u16, stats: [u16; 6] },
}

/// Highest base stat the `characters` columns hold.
const MAX_STAT: u32 = i16::MAX as u32;

/// Spends `points` (STR, DEX, REC, INT, WIS, LUC, as `stats`) out of `stat_point`.
pub fn spend(stat_point: u16, stats: [u16; 6], points: [u16; 6]) -> Spend {
    let sum: u32 = points.iter().map(|&p| u32::from(p)).sum();
    if sum == 0 {
        return Spend::Nothing;
    }
    let raised: [u32; 6] = std::array::from_fn(|i| u32::from(stats[i]) + u32::from(points[i]));
    if sum > u32::from(stat_point) || raised.iter().any(|&v| v > MAX_STAT) {
        return Spend::Refused;
    }
    Spend::Spent { stat_point: stat_point - sum as u16, stats: raised.map(|v| v as u16) }
}

/// The numbers from the base alone, then each equipped item in slots 0 to 12 (`CUser::InitEquipment`), then the passive skills
/// (`CUser::InitPassive`).
pub fn compute<'a>(
    table: &StatusTable,
    base: &Base,
    items: impl IntoIterator<Item = &'a ItemUnit>,
    defs: &ItemDefs,
    passives: &Passives,
) -> Totals {
    let [hp, sp, mp] = table.row(base.job, base.level);
    let mut t = Totals {
        stats: base.stats.map(i32::from),
        max_health: [scaled(hp, base.grow), scaled(mp, base.grow), scaled(sp, base.grow)],
        attack_speed: 0,
        move_speed: BASE_MOVE_SPEED,
        grade_recovery: [0; 3],
        grade_fight_recovery: [0; 3],
    };
    match base.family {
        0 => t.max_health[SP] += 10,
        1 => t.max_health[MP] += 10,
        2 => {
            t.max_health[SP] += 5;
            t.max_health[MP] += 5;
        }
        _ => t.max_health[HP] += 10,
    }
    t.max_health[HP] += i32::from(base.stats[REC]) * STAT_HEALTH;
    t.max_health[SP] += i32::from(base.stats[DEX]) * STAT_HEALTH;
    t.max_health[MP] += i32::from(base.stats[WIS]) * STAT_HEALTH;

    for item in items.into_iter().filter(|i| i.bag == 0 && i.slot < COUNTED_SLOTS) {
        let Some(def) = defs.get(item.item_type, item.type_id) else { continue };
        let add_stats = |stats: [i32; 6], t: &mut Totals| {
            for (total, add) in t.stats.iter_mut().zip(stats) {
                *total += add;
            }
            t.max_health[HP] += stats[REC] * STAT_HEALTH;
            t.max_health[SP] += stats[DEX] * STAT_HEALTH;
            t.max_health[MP] += stats[WIS] * STAT_HEALTH;
        };
        add_stats(def.const_stats.map(i32::from), &mut t);
        for (total, add) in t.max_health.iter_mut().zip(def.const_health) {
            *total += i32::from(add);
        }
        if item.slot == WEAPON_SLOT || ACCESSORY_SLOTS.contains(&item.slot) {
            t.attack_speed += i32::from(def.attack_time);
        }
        if item.slot == WEAPON_SLOT {
            t.attack_speed += Passives::weapon(&passives.weapon_speed, real_type(def.item_type));
        }
        if ACCESSORY_SLOTS.contains(&item.slot) {
            t.move_speed += i32::from(def.speed);
        }
        // Linked lapis (`CUser::GemEquipmentAdd`, `docs/original/lapis.md` section 2); a special of 16 sends the health to the
        // recovery numbers instead (no lapis has it).
        for lapis in crate::lapis::equipped_lapis(item, defs) {
            add_stats(lapis.const_stats.map(i32::from), &mut t);
            if lapis.special != RECOVERY_LAPIS {
                for (total, add) in t.max_health.iter_mut().zip(lapis.const_health) {
                    *total += i32::from(add);
                }
            }
            t.attack_speed += i32::from(lapis.attack_time);
            t.move_speed += i32::from(lapis.speed);
        }
        if def.craftable
            && let Some(craft) = craft_values(&item.craft_name)
        {
            add_stats([craft[0], craft[1], craft[2], craft[3], craft[4], craft[5]].map(i32::from), &mut t);
            t.max_health[HP] += i32::from(craft[6]) * CRAFT_HEALTH;
            t.max_health[MP] += i32::from(craft[7]) * CRAFT_HEALTH;
            t.max_health[SP] += i32::from(craft[8]) * CRAFT_HEALTH;
        }
    }
    faction_grade(&mut t, &base.grade);
    let b = &passives.bonuses;
    for (total, add) in t.stats.iter_mut().zip(b.stats) {
        *total += add;
    }
    for (total, add) in t.max_health.iter_mut().zip(b.max_health) {
        *total += add;
    }
    t.attack_speed += b.attack_speed;
    t.move_speed += b.move_speed;
    t
}

/// The faction grade's maxima, recovery and stats (`CUser::InitKCStatus` after the equipment, `docs/original/pvp-2.md` section 10 and
/// deviation 1): types 1 to 3 add percentages of the maxima so far, 10 to 15 recovery as percentages of those, then 4 to 9 the stats
/// (REC, WIS, DEX with their 5 HP, MP, SP each).
fn faction_grade(t: &mut Totals, g: &[i32; crate::kc::TYPES]) {
    use crate::kc::percent;
    let max = t.max_health;
    t.max_health[HP] += percent(max[HP], g[1]);
    t.max_health[MP] += percent(max[MP], g[2]);
    t.max_health[SP] += percent(max[SP], g[3]);
    let m = t.max_health;
    t.grade_recovery = [percent(m[HP], g[10]), percent(m[MP], g[12]), percent(m[SP], g[11])];
    t.grade_fight_recovery = [percent(m[HP], g[13]), percent(m[MP], g[15]), percent(m[SP], g[14])];
    // STR, DEX, REC, INT, WIS, LUC from types 4, 8, 5, 6, 7, 9.
    for (total, kind) in t.stats.iter_mut().zip([4, 8, 5, 6, 7, 9]) {
        *total += g[kind];
    }
    t.max_health[HP] += g[5] * STAT_HEALTH;
    t.max_health[MP] += g[7] * STAT_HEALTH;
    t.max_health[SP] += g[8] * STAT_HEALTH;
}

/// The first nine crafted values of a 20-character craft name: STR, DEX, REC, INT, WIS, LUC, HP, MP, SP (`CItem::CopyCraftExpassion`).
/// A pair that is not two digits counts as 0; a name of another length has none.
pub fn craft_values(craft_name: &[u8]) -> Option<[u16; 9]> {
    if craft_name.len() != 20 {
        return None;
    }
    let mut values = [0u16; 9];
    for (value, pair) in values.iter_mut().zip(craft_name.chunks(2)) {
        if let [a @ b'0'..=b'9', b @ b'0'..=b'9'] = pair {
            *value = u16::from(a - b'0') * 10 + u16::from(b - b'0');
        }
    }
    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemDef;

    fn table() -> StatusTable {
        // Fighter (0) and mage (4) at level 1, fighter at 60.
        StatusTable::new([(0, 1, [205, 125, 50]), (4, 1, [150, 60, 170]), (0, 60, [5000, 3000, 1000])])
    }

    #[test]
    fn the_faction_grade_adds_after_the_equipment() {
        let base = Base { job: 0, level: 1, grow: 1, family: 0, stats: [14, 12, 9, 8, 7, 15], ..Default::default() };
        let plain = compute(&table(), &base, [], &ItemDefs::default(), &Passives::default());
        let mut grade = [0; crate::kc::TYPES];
        (grade[1], grade[5], grade[8], grade[10], grade[11], grade[13], grade[9]) = (10, 2, 3, 5, 5, 1, 10);
        let t = compute(&table(), &Base { grade, ..base }, [], &ItemDefs::default(), &Passives::default());
        let [hp, mp, sp] = plain.max_health;
        assert_eq!(hp, 250);
        // 10 % of 250 is 24 with the original's 0.01f.
        let hp_after = hp + 24;
        assert_eq!(t.max_health, [hp_after + 10, mp, sp + 15]);
        assert_eq!(t.stats, [14, 15, 11, 8, 7, 25]);
        assert_eq!(t.grade_recovery, [crate::kc::percent(hp_after, 5), 0, crate::kc::percent(sp, 5)]);
        assert_eq!(t.grade_fight_recovery, [2, 0, 0]);
    }

    #[test]
    fn stat_points_are_spent_only_when_there_are_enough() {
        let stats = [14, 12, 9, 8, 7, 15];
        assert_eq!(spend(5, stats, [0; 6]), Spend::Nothing);
        assert_eq!(
            spend(5, stats, [1, 0, 2, 0, 0, 2]),
            Spend::Spent { stat_point: 0, stats: [15, 12, 11, 8, 7, 17] },
            "exactly all of them"
        );
        assert_eq!(spend(5, stats, [0, 3, 0, 0, 3, 0]), Spend::Refused);
        // Sums are not cut to 16 bits: 65535 + 2 is more than 5.
        assert_eq!(spend(5, stats, [u16::MAX, 2, 0, 0, 0, 0]), Spend::Refused);
        assert_eq!(spend(u16::MAX, [32760, 0, 0, 0, 0, 0], [8, 0, 0, 0, 0, 0]), Spend::Refused, "past the column");
        assert_eq!(
            spend(u16::MAX, [32760, 0, 0, 0, 0, 0], [7, 0, 0, 0, 0, 0]),
            Spend::Spent { stat_point: u16::MAX - 7, stats: [32767, 0, 0, 0, 0, 0] }
        );
    }

    #[test]
    fn maxima_follow_class_level_mode_race_and_stats() {
        // Human fighter, normal mode: HP 205 + 9*5, SP 125 + 10 + 12*5, MP 50 + 7*5 (the values character creation gives too).
        let fighter = Base { job: 0, level: 1, grow: 1, family: 0, stats: [14, 12, 9, 8, 7, 15], ..Default::default() };
        let t = compute(&table(), &fighter, [], &ItemDefs::default(), &Passives::default());
        assert_eq!(t.max_health(), [250, 85, 195], "HP, MP, SP");
        assert_eq!(t.speeds(), (0, 2));
        // Hard mode: 205 * 1.1f = 225.5 truncated.
        assert_eq!(
            compute(&table(), &Base { grow: 2, ..fighter }, [], &ItemDefs::default(), &Passives::default()).max_health()[0],
            225 + 45
        );
        // Deatheater mage in ultimate mode: HP 150 * 1.5 + 10 + REC * 5.
        let mage = Base { job: 4, level: 1, grow: 3, family: 3, stats: [10, 10, 10, 10, 10, 10], ..Default::default() };
        assert_eq!(
            compute(&table(), &mage, [], &ItemDefs::default(), &Passives::default()).max_health(),
            [225 + 10 + 50, 255 + 50, 90 + 50]
        );
        // A level outside the table takes class 0 level 60.
        assert_eq!(
            compute(&table(), &Base { level: 61, ..fighter }, [], &ItemDefs::default(), &Passives::default()).max_health()[0],
            5000 + 45
        );
    }

    #[test]
    fn equipment_adds_its_bonuses() {
        let item = |slot, item_type| ItemUnit { bag: 0, slot, item_type, type_id: 1, count: 1, ..Default::default() };
        let defs = ItemDefs::new([
            // A helmet with REC 2, HP 30 and crafted stats; a weapon with attack speed 4; a ring with move speed 1.
            ItemDef {
                item_type: 16,
                type_id: 1,
                const_stats: [0, 0, 2, 0, 0, 0],
                const_health: [30, 0, 0],
                craftable: true,
                ..Default::default()
            },
            ItemDef { item_type: 1, type_id: 1, attack_time: 4, ..Default::default() },
            ItemDef { item_type: 22, type_id: 1, speed: 1, ..Default::default() },
            ItemDef { item_type: 42, type_id: 1, const_health: [999, 0, 0], ..Default::default() },
        ]);
        let base = Base { job: 0, level: 1, grow: 1, family: 0, stats: [14, 12, 9, 8, 7, 15], ..Default::default() };
        // Crafted: DEX 3, HP 1 (100), SP 2 (200).
        let helmet = ItemUnit { craft_name: b"00030000000001000200".to_vec(), ..item(0, 16) };
        let items = [helmet, item(5, 1), item(9, 22), item(13, 42), ItemUnit { bag: 1, ..item(0, 16) }];
        let t = compute(&table(), &base, &items, &defs, &Passives::default());
        assert_eq!(t.stats, [14, 15, 11, 8, 7, 15]);
        // HP 250 + 30 + 2*5 + 100; MP 85; SP 195 + 3*5 + 200. The mount (slot 13) and bag 1 do not count.
        assert_eq!(t.max_health(), [390, 85, 410]);
        assert_eq!(t.speeds(), (4, 3));
    }

    #[test]
    fn lapis_add_stats_health_and_speeds() {
        // A helmet with a lapis of STR 3, REC 2, HP 50, attack speed 1; the same lapis in the cloak (its badge) adds nothing.
        let defs = ItemDefs::new([
            ItemDef { item_type: 16, type_id: 1, ..Default::default() },
            ItemDef { item_type: 24, type_id: 1, ..Default::default() },
            ItemDef {
                item_type: 30,
                type_id: 9,
                const_stats: [3, 0, 2, 0, 0, 0],
                const_health: [50, 0, 0],
                attack_time: 1,
                ..Default::default()
            },
        ]);
        let base = Base { job: 0, level: 1, grow: 1, family: 0, stats: [14, 12, 9, 8, 7, 15], ..Default::default() };
        let table = StatusTable::new([(0, 1, [205, 125, 50])]);
        let item =
            |slot, item_type| ItemUnit { bag: 0, slot, item_type, type_id: 1, count: 1, gems: [9, 0, 0, 0, 0, 0], ..Default::default() };
        let without = compute(&table, &base, &[], &defs, &Passives::default());
        let with = compute(&table, &base, &[item(0, 16), item(7, 24)], &defs, &Passives::default());
        assert_eq!(with.stats[0] - without.stats[0], 3);
        assert_eq!(with.max_health[HP] - without.max_health[HP], 50 + 2 * 5);
        assert_eq!(with.attack_speed - without.attack_speed, 1);
    }

    #[test]
    fn passive_skills_add_after_the_equipment() {
        let defs = ItemDefs::new([ItemDef { item_type: 1, type_id: 1, attack_time: 4, ..Default::default() }]);
        let base = Base { job: 0, level: 1, grow: 1, family: 0, stats: [14, 12, 9, 8, 7, 15], ..Default::default() };
        let mut passives = Passives::default();
        passives.weapon_speed[1] = 1;
        passives.weapon_speed[2] = 3;
        passives.bonuses.add(5, 10, 0);
        passives.bonuses.add(2, 20, 0);
        passives.bonuses.add(17, 1, 0);
        let weapon = ItemUnit { bag: 0, slot: 5, item_type: 1, type_id: 1, count: 1, ..Default::default() };
        let t = compute(&table(), &base, [&weapon], &defs, &passives);
        assert_eq!(t.stats[2], 19, "REC");
        assert_eq!(t.max_health(), [250 + 50, 85 + 20, 195]);
        assert_eq!(t.speeds(), (5, 3), "only the worn weapon's real type counts");
        assert_eq!(compute(&table(), &base, [], &defs, &passives).speeds(), (0, 3), "no weapon, no weapon speed");
    }

    #[test]
    fn craft_names_are_ten_pairs() {
        assert_eq!(craft_values(b"01020304050607080910"), Some([1, 2, 3, 4, 5, 6, 7, 8, 9]));
        assert_eq!(craft_values(b"0102030405060708x910"), Some([1, 2, 3, 4, 5, 6, 7, 8, 0]), "not two digits: 0");
        assert_eq!(craft_values(b"0102"), None);
    }
}
