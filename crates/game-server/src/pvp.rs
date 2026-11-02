//! Player kills (`docs/original/pvp.md`): the experience a victim gives, the kill and death counters, the reward steps of the
//! `0x0218` to `0x021A` panels, and what a victim loses. Pure functions; the zone and the connection apply them.

use crate::backend::ItemUnit;
use crate::inventory::{ItemDefs, real_type};

/// Kills and deaths stop counting here (`g_nMaxKillValue`, `g_nMaxDeathValue`).
pub const MAX_COUNT: u32 = 200_000;
/// The kill and death reward steps (`g_nMaxKillGrow` is 16 for every mode).
pub const STEPS: usize = 16;
/// Below this level the panels are not answered (`CUser::PacketMaininterface`).
pub const PANEL_LEVEL: u16 = 16;
/// Items of this type never go on a death (the guild gold bars).
const NO_LOSS_TYPE: u8 = 94;
/// Real types a player can lose: weapons, armour and accessories.
const LOSS_REAL_TYPES: std::ops::RangeInclusive<u8> = 1..=25;
/// The victim loses nothing at or below this level.
const LOSS_LEVEL: u16 = 10;
/// The first items found that a player or a staff member may lose, in bag order.
const PLAYER_CANDIDATES: usize = 10;
const STAFF_CANDIDATES: usize = 25;

/// The kills each step needs (the binary's table at 0x541d74) and the stat points it gives in normal, hard and ultimate mode.
const KILL_STEPS: [(u32, [u8; 3]); STEPS] = [
    (1, [1, 1, 1]),
    (50, [2, 3, 3]),
    (300, [3, 5, 5]),
    (1000, [5, 7, 7]),
    (5000, [10, 12, 12]),
    (10000, [15, 15, 18]),
    (20000, [16, 18, 20]),
    (30000, [16, 21, 22]),
    (40000, [16, 21, 24]),
    (50000, [17, 22, 26]),
    (70000, [17, 22, 26]),
    (90000, [17, 22, 26]),
    (110000, [18, 23, 27]),
    (130000, [18, 23, 27]),
    (150000, [18, 23, 27]),
    (200000, [19, 24, 28]),
];

/// The deaths each step needs (0x541eb4) and the gold it gives.
const DEATH_STEPS: [(u32, u32); STEPS] = [
    (1, 100),
    (50, 200),
    (300, 500),
    (1000, 1000),
    (5000, 5000),
    (10000, 10000),
    (20000, 20000),
    (30000, 50000),
    (40000, 100000),
    (50000, 200000),
    (70000, 300000),
    (90000, 400000),
    (110000, 500000),
    (130000, 650000),
    (150000, 800000),
    (200000, 1000000),
];

/// The experience a victim of level `victim` gives a player of level `level` (`CUser::GetExp` 0x4581b0): nothing when the victim
/// is more than 9 levels above, otherwise at least 1.
pub fn victim_exp(victim: u16, level: u16) -> u32 {
    let d = i32::from(victim) - i32::from(level);
    if d > 9 {
        return 0;
    }
    let exp = f64::from(50 * u32::from(victim)) * (f64::from(d + 1) * 0.2 + 1.0);
    (exp as i64).max(1) as u32
}

/// What the killer gets alone (`CUser::Death`): the victim's experience × 1.2.
pub fn solo_exp(victim: u16, level: u16) -> u32 {
    (f64::from(victim_exp(victim, level)) * 1.2) as u32
}

/// A counter after `by` more (`KillPCTo`, `KillPCFrom`: 1; the kill rate and the kill boost of
/// `docs/original/operator-console.md` section 4 give a kill more): `None` at the cap, where nothing changes; else at most the cap.
pub fn counted(count: u32, by: u32) -> Option<u32> {
    (count < MAX_COUNT).then(|| count.saturating_add(by).min(MAX_COUNT))
}

/// The count a saved step stands for (`GetKillStepCount`, `GetDeathStepCount`): the kills or deaths at which the reward was last
/// taken. Step 0 and steps past 16 are 0; kill steps are 0 in easy mode.
pub fn step_count(kills: bool, grow: u8, step: u8) -> u32 {
    let step = usize::from(step);
    if !(1..=STEPS).contains(&step) || (kills && grow == 0) {
        return 0;
    }
    if kills { KILL_STEPS[step - 1].0 } else { DEATH_STEPS[step - 1].0 }
}

/// How many steps a used count has passed: the next step is this + 1.
fn passed(thresholds: impl Iterator<Item = u32>, used: u32) -> usize {
    thresholds.take_while(|&t| t <= used).count()
}

/// `0x0218`: the next kill step and the next death step (`GetKillStepEnable`, `GetDeathStepEnable`). When every step is taken the
/// kill side says 17 and the death side 11 (the original's quirk).
pub fn next_steps(grow: u8, kill_steps: [u8; 2]) -> (u8, u8) {
    let kill = if grow == 0 { 0 } else { passed(KILL_STEPS.iter().map(|s| s.0), step_count(true, grow, kill_steps[0])) as u8 + 1 };
    let death = match passed(DEATH_STEPS.iter().map(|s| s.0), step_count(false, grow, kill_steps[1])) {
        STEPS => 11,
        n => n as u8 + 1,
    };
    (kill, death)
}

/// A reward the panel gives: the step it reaches and what it is worth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reward {
    pub step: u8,
    pub value: u32,
}

/// What `0x0219` or `0x021A` does (`ChkKillResult`, `ChkDeathResult`): `None` when the count equals the used count (no answer at
/// all); `Some(None)` when the next step is not reached (`0, 0`); otherwise the next step and its stat points (kills, by mode) or gold
/// (deaths). One step per request.
pub fn claim(kills: bool, grow: u8, count: u32, step: u8) -> Option<Option<Reward>> {
    let used = step_count(kills, grow, step);
    if count == used {
        return None;
    }
    let next = if kills {
        let mode = match grow {
            1..=3 => usize::from(grow) - 1,
            _ => return Some(None),
        };
        KILL_STEPS.iter().map(|&(t, points)| (t, u32::from(points[mode]))).enumerate().find(|&(_, (t, _))| t > used)
    } else {
        DEATH_STEPS.iter().copied().enumerate().find(|&(_, (t, _))| t > used)
    };
    Some(match next {
        Some((i, (threshold, value))) if count >= threshold => Some(Reward { step: i as u8 + 1, value }),
        _ => None,
    })
}

/// The chance in percent that a player loses an item on a death, by its VG (`CUser::ItemDropByUser`).
pub fn loss_chance(vg: i16) -> u32 {
    match vg.unsigned_abs() {
        191.. => 65,
        171..=190 => 70,
        141..=170 => 80,
        101..=140 => 85,
        51..=100 => 90,
        11..=50 => 95,
        _ => 100,
    }
}

/// The items a victim may lose, as `(bag, slot)` in bag order: in bags 1 to `bags`, the first 10 weapons, armour and accessories that
/// are not marked OG and are not guild gold bars (a player), or the first 25 items but the gold bars (staff).
pub fn loss_candidates<'a>(items: impl IntoIterator<Item = &'a ItemUnit>, bags: u8, staff: bool, defs: &ItemDefs) -> Vec<(u8, u8)> {
    let mut found: Vec<&ItemUnit> = items.into_iter().filter(|i| (1..=bags).contains(&i.bag) && i.slot < 24).collect();
    found.sort_by_key(|i| (i.bag, i.slot));
    let fits = |i: &ItemUnit| {
        if i.item_type == NO_LOSS_TYPE {
            return false;
        }
        staff || defs.get(i.item_type, i.type_id).is_some_and(|d| LOSS_REAL_TYPES.contains(&real_type(d.item_type)) && !d.bound)
    };
    let limit = if staff { STAFF_CANDIDATES } else { PLAYER_CANDIDATES };
    found.into_iter().filter(|i| fits(i)).take(limit).map(|i| (i.bag, i.slot)).collect()
}

/// Whether a victim of this level can lose anything.
pub fn can_lose(level: u16) -> bool {
    level > LOSS_LEVEL
}

/// The gold a victim loses when the 30 % roll falls (`rand() % (5 × level) + 1`, at most its money).
pub fn gold_loss(level: u16, money: u32, roll: u32) -> u32 {
    (roll % (5 * u32::from(level).max(1)) + 1).min(money)
}

/// Each party member's part of a victim's gold (`CUser::ItemDropByUser`): the loss over all members, at least 1.
pub fn gold_part(loss: u32, members: usize) -> u32 {
    ((loss as f32 / members.max(1) as f32) as u32).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn victims_give_experience_by_their_level() {
        assert_eq!(victim_exp(30, 30), 1800, "50 × 30 × 1.2");
        assert_eq!(victim_exp(40, 30), 0, "ten above gives nothing");
        assert_eq!(victim_exp(39, 30), 5850, "50 × 39 × 3.0");
        assert_eq!(victim_exp(11, 60), 1, "far below still gives 1");
        assert_eq!(solo_exp(30, 30), 2160);
    }

    #[test]
    fn counters_stop_at_the_cap() {
        assert_eq!(counted(0, 1), Some(1));
        assert_eq!(counted(MAX_COUNT - 1, 1), Some(MAX_COUNT));
        assert_eq!(counted(MAX_COUNT, 1), None);
        assert_eq!(counted(MAX_COUNT - 1, 2), Some(MAX_COUNT), "the kill rate stops at the cap");
        assert_eq!(counted(5, 6), Some(11));
    }

    #[test]
    fn panels_show_the_next_steps() {
        assert_eq!(next_steps(1, [0, 0]), (1, 1));
        assert_eq!(next_steps(2, [3, 1]), (4, 2));
        assert_eq!(next_steps(1, [16, 16]), (17, 11), "the original's quirk on the death side");
        assert_eq!(next_steps(0, [5, 0]), (0, 1), "easy mode has no kill steps");
    }

    #[test]
    fn rewards_come_one_step_at_a_time() {
        assert_eq!(claim(true, 1, 0, 0), None, "kills equal to the used count: no answer");
        assert_eq!(claim(true, 1, 60, 0), Some(Some(Reward { step: 1, value: 1 })), "one step even with enough for two");
        assert_eq!(claim(true, 3, 60, 1), Some(Some(Reward { step: 2, value: 3 })));
        assert_eq!(claim(true, 2, 40, 1), Some(None), "50 not reached");
        assert_eq!(claim(true, 1, 300_000, 16), Some(None), "every step taken");
        assert_eq!(claim(false, 1, 1, 0), Some(Some(Reward { step: 1, value: 100 })));
        assert_eq!(claim(false, 1, 1000, 3), Some(Some(Reward { step: 4, value: 1000 })));
        assert_eq!(step_count(true, 1, 4), 1000);
        assert_eq!(step_count(true, 0, 4), 0, "easy mode loads no kill step");
        assert_eq!(step_count(false, 2, 17), 0);
    }

    #[test]
    fn losses_by_vg_and_gold() {
        assert_eq!((loss_chance(0), loss_chance(-11), loss_chance(191), loss_chance(i16::MIN)), (100, 95, 65, 65));
        assert_eq!(gold_loss(20, 1000, 99), 100, "1 to 100 at level 20");
        assert_eq!(gold_loss(20, 30, 99), 30, "at most the money");
        assert_eq!((gold_part(100, 3), gold_part(2, 7)), (33, 1));
        assert!(!can_lose(10) && can_lose(11));
    }
}
