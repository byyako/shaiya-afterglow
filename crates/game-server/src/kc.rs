//! The faction kill count (`docs/original/pvp-2.md` sections 8 to 10): each faction's player kills, the grades of
//! `KillStatus.SData` they reach and what a grade gives the faction's players, the top grade's ten minutes and the resets. Pure; the
//! server keeps one `KillCounts` and acts on the `Changes` it returns.

use std::time::{Duration, Instant};

/// Bonus types 0 to 27 (`stKCStatus::pStatus`).
pub const TYPES: usize = 28;
/// The two factions.
pub const FACTIONS: usize = 2;
/// The top grade's time, the time between two `0x020F`, between two saves (`600000` ms throughout `CWorld`).
pub const PERIOD: Duration = Duration::from_secs(600);
/// What the Goddess Blessing adds (`CUser::ItemUse`, special 30).
pub const BLESSING_GRADES: usize = 5;

/// A `world.kill_status` row as imported: the faction, the kills of the grade and up to six bonuses (type, value) in file order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub faction: u8,
    pub kills: u32,
    pub bonuses: [(u8, i32); 6],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grade {
    /// The count that reaches it (`dwKillCount`).
    pub kills: u32,
    /// What it gives, by type (`pStatus`), with the grades below folded in.
    pub status: [i32; TYPES],
}

/// Each faction's grades (`CGameData::LoadKCStatus`): grade 0 with nothing, then the rows of that faction in file order; types 1 to
/// 27 only; then each grade takes the larger of its own value and the one below's, type by type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grades {
    factions: [Vec<Grade>; FACTIONS],
}

/// Grade 0 alone for each faction: kills change nothing.
impl Default for Grades {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl Grades {
    pub fn new(rows: &[Row]) -> Self {
        let empty = Grade { kills: 0, status: [0; TYPES] };
        let mut factions = [vec![empty], vec![empty]];
        for row in rows.iter().filter(|r| usize::from(r.faction) < FACTIONS) {
            let mut status = [0; TYPES];
            for &(kind, value) in &row.bonuses {
                if (1..TYPES).contains(&usize::from(kind)) {
                    status[usize::from(kind)] = value;
                }
            }
            factions[usize::from(row.faction)].push(Grade { kills: row.kills, status });
        }
        for grades in &mut factions {
            for i in 1..grades.len() {
                let below = grades[i - 1].status;
                for (v, b) in grades[i].status.iter_mut().zip(below) {
                    *v = (*v).max(b);
                }
            }
        }
        Self { factions }
    }

    fn of(&self, faction: usize) -> &[Grade] {
        &self.factions[faction]
    }

    /// The top grade (`GetKCStatusMax`).
    pub fn top(&self, faction: usize) -> usize {
        self.of(faction).len() - 1
    }

    /// The highest grade whose kills are at most `count` (`GetKCStatusByCount`).
    pub fn by_count(&self, faction: usize, count: u32) -> usize {
        self.of(faction).iter().rposition(|g| g.kills <= count).unwrap_or(0)
    }

    pub fn get(&self, faction: usize, grade: usize) -> &Grade {
        &self.of(faction)[grade.min(self.top(faction))]
    }
}

/// One faction's place (`stKCInfo`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Faction {
    grade: usize,
    count: u32,
    /// The count of the next grade (`dwCountNext`).
    next: u32,
    /// The top grade's end (`dwResetTime`).
    reset_at: Option<Instant>,
    /// The next `0x020F` (`dwSendTime`).
    send_at: Instant,
}

/// What a change asks the server to do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// The faction's grade was set again: its players' numbers follow (`CZone::ApplyKillCountAllUser`), then `0x020F`.
    pub applied: [bool; FACTIONS],
    /// `0x020F` to the faction's players.
    pub sent: [bool; FACTIONS],
    /// The counts go to the database (the original's `0x0205` to ps_dbAgent).
    pub save: bool,
    /// A faction reached its top grade by a kill, a capture or the Goddess Blessing (`ApplyKillCount(country, true)`): the obelisks
    /// are reset (`docs/original/obelisks.md` section 5).
    pub top: bool,
    /// The faction that reached it (the game log's `0x0A01`).
    pub top_by: Option<u8>,
}

impl Changes {
    pub fn any(&self) -> bool {
        self.applied.iter().chain(&self.sent).any(|&b| b) || self.save || self.top
    }

    /// Both changes' requests.
    pub fn merge(&mut self, other: Changes) {
        for f in 0..FACTIONS {
            self.applied[f] |= other.applied[f];
            self.sent[f] |= other.sent[f];
        }
        self.save |= other.save;
        self.top |= other.top;
        self.top_by = self.top_by.or(other.top_by);
    }
}

/// The world's kill counts (`CWorld::m_KCInfo`).
#[derive(Clone, Debug)]
pub struct KillCounts {
    grades: Grades,
    factions: [Faction; FACTIONS],
    /// `g_bDisableKillCount`.
    pub disabled: bool,
}

fn other(faction: usize) -> usize {
    1 - faction
}

impl KillCounts {
    /// The counts as loaded (`CWorld::InitKillCount`): each faction's grade and next count, the top grade's ten minutes from now.
    /// The first tick sends `0x020F` (the send time starts at 0 there).
    pub fn new(grades: Grades, counts: [u32; FACTIONS], now: Instant) -> Self {
        let blank = Faction { grade: 0, count: 0, next: 0, reset_at: None, send_at: now };
        let mut k = Self { grades, factions: [blank; FACTIONS], disabled: false };
        for (f, count) in counts.into_iter().enumerate() {
            k.factions[f].count = count;
            k.regrade(f, now);
        }
        k
    }

    /// The grade by the count, the next count, or at the top the reset time; whether that is the top.
    fn regrade(&mut self, f: usize, now: Instant) -> bool {
        let grade = self.grades.by_count(f, self.factions[f].count);
        let top = grade >= self.grades.top(f);
        let place = &mut self.factions[f];
        place.grade = grade;
        if top {
            place.reset_at = Some(now + PERIOD);
        } else {
            place.next = self.grades.get(f, grade + 1).kills;
        }
        top
    }

    /// Counting stops while counting is off or either faction is in its top grade's ten minutes.
    fn frozen(&self) -> bool {
        self.disabled || self.factions.iter().any(|p| p.reset_at.is_some())
    }

    /// A count that reached the next grade (`CWorld::AddKillCount` after the addition): the grade, `ApplyKillCount`, the send time;
    /// at the top the other faction goes back to 0 (`CWorld::ResetKillCount`).
    fn reached(&mut self, f: usize, now: Instant, changes: &mut Changes) {
        if self.factions[f].count < self.factions[f].next {
            return;
        }
        let top = self.regrade(f, now);
        changes.applied[f] = true;
        self.factions[f].send_at = now + PERIOD;
        if top {
            changes.top = true;
            changes.top_by = Some(f as u8);
            let o = other(f);
            self.set(o, 0, now, changes);
            changes.sent[o] = true;
            self.factions[o].send_at = now + PERIOD;
            self.factions[o].reset_at = None;
            changes.save = true;
        }
    }

    /// `CWorld::SetKillCount`: nothing when equal; otherwise the count, its grade (at the top the reset time), `ApplyKillCount` and
    /// the send time.
    fn set(&mut self, f: usize, count: u32, now: Instant, changes: &mut Changes) {
        if self.factions[f].count == count {
            return;
        }
        self.factions[f].count = count;
        self.regrade(f, now);
        changes.applied[f] = true;
        self.factions[f].send_at = now + PERIOD;
    }

    /// A player of `faction` killed one of the other (`CWorld::AddKillCount(country, n)`).
    pub fn add(&mut self, faction: u8, n: u32, now: Instant) -> Changes {
        let mut changes = Changes::default();
        let f = usize::from(faction);
        if f >= FACTIONS || self.frozen() {
            return changes;
        }
        self.factions[f].count = self.factions[f].count.saturating_add(n);
        self.reached(f, now, &mut changes);
        changes
    }

    /// A faction lost an obelisk it held (`CWorld::SubKillCount(country, n)`, `docs/original/obelisks.md` section 6): nothing while
    /// counting is frozen; otherwise `n`, at most the count, is taken off; a count below its grade's kills takes the grade by the
    /// count again and sets it (deviation 2: the original waits until the count is below the grade under that one). No save.
    pub fn sub(&mut self, faction: u8, n: u32, now: Instant) -> Changes {
        let mut changes = Changes::default();
        let f = usize::from(faction);
        if f >= FACTIONS || self.frozen() {
            return changes;
        }
        let place = &mut self.factions[f];
        place.count -= n.min(place.count);
        if place.grade > 0 && place.count < self.grades.get(f, place.grade).kills {
            let grade = self.grades.by_count(f, place.count);
            place.grade = grade;
            place.next = self.grades.get(f, grade + 1).kills;
            place.send_at = now + PERIOD;
            changes.applied[f] = true;
        }
        changes
    }

    /// Whether the faction is in its top grade's ten minutes (`dwResetTime` not 0).
    pub fn in_top(&self, faction: u8) -> bool {
        self.factions.get(usize::from(faction)).is_some_and(|p| p.reset_at.is_some())
    }

    /// Whether a Blessing would be accepted now: the faction exists and counting is not frozen (audit R13: checked before the item is
    /// used up, and again by [`Self::add_grade`] when it takes effect).
    pub fn can_add_grade(&self, faction: u8) -> bool {
        usize::from(faction) < FACTIONS && !self.frozen()
    }

    /// The Goddess Blessing (`CWorld::AddKillCountGrade(country, n)`): `None` (refused) while counting is frozen; otherwise the count
    /// becomes that of `n` grades higher (at most the top) and goes on as a kill.
    pub fn add_grade(&mut self, faction: u8, n: usize, now: Instant) -> Option<Changes> {
        let f = usize::from(faction);
        if f >= FACTIONS || self.frozen() {
            return None;
        }
        let mut changes = Changes::default();
        let grade = (self.factions[f].grade + n).min(self.grades.top(f));
        self.factions[f].count = self.grades.get(f, grade).kills;
        self.reached(f, now, &mut changes);
        Some(changes)
    }

    /// The world tick (`CWorld::UpdateKillCount`): each faction's `0x020F` every ten minutes; the top grade's ten minutes over, its
    /// count back to 0. Anything done saves the counts.
    pub fn tick(&mut self, now: Instant) -> Changes {
        let mut changes = Changes::default();
        let mut done = false;
        for f in 0..FACTIONS {
            match self.factions[f].reset_at {
                None if now >= self.factions[f].send_at => {}
                Some(at) if now >= at => {
                    self.set(f, 0, now, &mut changes);
                    self.factions[f].reset_at = None;
                }
                _ => continue,
            }
            changes.sent[f] = true;
            self.factions[f].send_at = now + PERIOD;
            done = true;
        }
        changes.save |= done;
        changes
    }

    pub fn counts(&self) -> [u32; FACTIONS] {
        [self.factions[0].count, self.factions[1].count]
    }

    /// What the faction's grade gives its players.
    pub fn status(&self, faction: u8) -> [i32; TYPES] {
        let f = usize::from(faction);
        if f >= FACTIONS {
            return [0; TYPES];
        }
        self.grades.get(f, self.factions[f].grade).status
    }

    /// `0x020F`'s count: at most the top grade's kills (`CWorldThread::SendAllKillCount`).
    pub fn shown(&self, faction: u8) -> u32 {
        let f = usize::from(faction).min(FACTIONS - 1);
        self.factions[f].count.min(self.grades.get(f, self.grades.top(f)).kills)
    }

    /// `0x0211`'s count and the milliseconds left of the top grade (`CUser::SendKCStatus`).
    pub fn status_packet_values(&self, faction: u8, now: Instant) -> (u32, u32) {
        let f = usize::from(faction).min(FACTIONS - 1);
        let p = &self.factions[f];
        let left = p.reset_at.map_or(0, |at| at.saturating_duration_since(now).as_millis().min(u128::from(u32::MAX)) as u32);
        (p.count, left)
    }
}

/// `trunc(f32(value × percent) × 0.01f)`, the original's percentages of the grade (`CUser::InitKCStatus`): `0.01f` is a little
/// below 0.01, so a round product loses one (6 % of 1000 is 59), as there.
pub fn percent(value: i32, percent: i32) -> i32 {
    (f64::from(value.saturating_mul(percent) as f32) * f64::from(0.01f32)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(faction: u8, kills: u32, bonuses: &[(u8, i32)]) -> Row {
        let mut b = [(0, 0); 6];
        b[..bonuses.len()].copy_from_slice(bonuses);
        Row { faction, kills, bonuses: b }
    }

    fn grades() -> Grades {
        Grades::new(&[
            row(0, 150, &[(11, 5), (12, 5)]),
            row(1, 150, &[(1, 6)]),
            row(0, 300, &[(10, 5)]),
            row(1, 300, &[(10, 5)]),
            row(0, 450, &[(11, 3), (24, 1)]),
            row(1, 450, &[]),
            row(0, 600, &[(28, 9), (0, 9)]),
        ])
    }

    #[test]
    fn grades_build_up_and_ignore_unknown_types() {
        let g = grades();
        assert_eq!((g.top(0), g.top(1)), (4, 3));
        assert_eq!(g.get(0, 0).status, [0; TYPES]);
        let third = g.get(0, 3).status;
        assert_eq!((third[10], third[11], third[12], third[24]), (5, 5, 5, 1), "the larger of its own and the grades below");
        assert_eq!(g.get(0, 4).status, third, "types 0 and 28 are not taken");
        assert_eq!((g.by_count(0, 0), g.by_count(0, 149), g.by_count(0, 150), g.by_count(0, 9999)), (0, 0, 1, 4));
        assert_eq!(g.get(1, 2).status[1], 6);
    }

    #[test]
    fn kills_climb_the_grades_and_the_top_resets_the_other_faction() {
        let t0 = Instant::now();
        let mut k = KillCounts::new(grades(), [0, 299], t0);
        assert_eq!(k.add(0, 149, t0), Changes::default());
        assert_eq!(k.add(0, 1, t0).applied, [true, false]);
        assert_eq!(k.status(0)[11], 5);
        assert_eq!(k.add(1, 1, t0).applied, [false, true], "Fury reaches grade 2 at 300");
        // Light to the top: Fury goes back to 0, counting stops for both for ten minutes.
        let c = k.add(0, 450, t0);
        assert_eq!(c, Changes { applied: [true, true], sent: [false, true], save: true, top: true, top_by: Some(0) });
        assert_eq!(k.counts(), [600, 0]);
        assert_eq!(k.status(1), [0; TYPES]);
        assert_eq!(k.status_packet_values(0, t0 + Duration::from_secs(60)), (600, 540_000));
        assert_eq!(k.add(1, 5, t0), Changes::default(), "frozen");
        assert_eq!(k.add_grade(1, BLESSING_GRADES, t0), None);
        assert!(!k.can_add_grade(1) && !k.can_add_grade(0), "the check before the item is used up agrees");
        assert!(!KillCounts::new(grades(), [0, 0], t0).can_add_grade(2), "no such faction");
        assert!(KillCounts::new(grades(), [0, 0], t0).can_add_grade(1));
        // Ten minutes later Light's count goes back to 0.
        let c = k.tick(t0 + PERIOD);
        assert_eq!(c, Changes { applied: [true, false], sent: [true, true], save: true, ..Default::default() });
        assert_eq!(k.counts(), [0, 0]);
        assert_eq!(k.add(1, 1, t0 + PERIOD).applied, [false, false], "counting again");
        assert_eq!(k.counts(), [0, 1]);
    }

    #[test]
    fn the_tick_sends_every_ten_minutes_and_saves() {
        let t0 = Instant::now();
        let mut k = KillCounts::new(grades(), [10, 20], t0);
        assert_eq!(k.tick(t0), Changes { applied: [false; 2], sent: [true; 2], save: true, ..Default::default() }, "at once after loading");
        assert_eq!(k.tick(t0 + Duration::from_secs(599)), Changes::default());
        assert!(k.tick(t0 + PERIOD).save);
        assert_eq!(k.shown(0), 10);
    }

    #[test]
    fn a_lost_obelisk_takes_its_worth_off() {
        let t0 = Instant::now();
        let mut k = KillCounts::new(grades(), [460, 100], t0);
        assert_eq!(k.status(0)[24], 1, "Light at grade 3 (450)");
        // Below grade 3's 450 the grade goes by the count (deviation 2): 300 is grade 2.
        let c = k.sub(0, 160, t0);
        assert_eq!((c.applied, c.save, k.counts()[0]), ([true, false], false, 300));
        assert_eq!(k.status(0)[24], 0);
        // Still grade 2's: only the count.
        assert_eq!(k.sub(0, 0, t0), Changes::default());
        assert_eq!(k.sub(1, 500, t0).applied, [false, false], "cut to the count; grade 0 stays");
        assert_eq!(k.counts(), [300, 0]);
        assert!(!k.in_top(0));
        k.add(0, 300, t0);
        assert!(k.in_top(0));
        assert_eq!(k.sub(1, 1, t0), Changes::default(), "frozen");
        let mut both = Changes { sent: [true, false], ..Default::default() };
        both.merge(Changes { applied: [false, true], top: true, ..Default::default() });
        assert_eq!(both, Changes { applied: [false, true], sent: [true, false], save: false, top: true, top_by: None });
    }

    #[test]
    fn a_blessing_raises_five_grades_at_most_to_the_top() {
        let t0 = Instant::now();
        let mut k = KillCounts::new(grades(), [0, 0], t0);
        let c = k.add_grade(1, BLESSING_GRADES, t0).expect("not frozen");
        assert_eq!(k.counts(), [0, 450], "Fury's top is grade 3");
        // Light's count is already 0: only its 0x020F.
        assert_eq!(c, Changes { applied: [false, true], sent: [true, false], save: true, top: true, top_by: Some(1) });
        k.disabled = true;
        assert_eq!(k.add(0, 1, t0), Changes::default());
        // Loaded at the top: its ten minutes start now.
        let k = KillCounts::new(grades(), [600, 0], t0);
        assert_eq!(k.status_packet_values(0, t0).1, 600_000);
        assert_eq!(k.shown(0), 600);
    }

    #[test]
    fn percentages_truncate_in_single_precision() {
        assert_eq!(percent(1000, 6), 59);
        assert_eq!(percent(1234, 5), 61);
        assert_eq!(percent(0, 50), 0);
    }
}
