//! The guild ranking battle's clock and arithmetic (`docs/original/guild-ranking.md`, pure).

/// The R map's open times that count: only the first two entries are read (`IsGuildRankMapOpenTime`).
const ENTRIES: usize = 2;

/// The highest place that keeps a ranking.
pub const PLACES: usize = 30;

/// `0x0D26` types for the battle's start, its 10 minutes and its last minute.
pub const NOTICE_STARTED: u8 = 100;
pub const NOTICE_TEN_MINUTES: u8 = 101;
pub const NOTICE_ONE_MINUTE: u8 = 102;

/// A moment of the server's local clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moment {
    /// 0 Sunday.
    pub weekday: u16,
    pub hour: u16,
    pub minute: u16,
    /// The calendar day as a number that grows by one a day (days from the Common Era).
    pub date: u32,
}

/// One battle's name: the day it started on and its start hour. A player who entered it may not enter it again.
pub type Tag = u64;

fn tag(date: u32, hour: u16) -> Tag {
    u64::from(date) << 16 | u64::from(hour)
}

/// The battle that is open at the moment, if any, as its tag: on an entry's weekday from its hour while the hour is below
/// `hour + hours`, an entry that runs past 24 also on the next weekday (`CMap::IsGuildRankMapOpenTime`). No entries: never open.
pub fn open_tag(times: &[[u16; 3]], at: Moment) -> Option<Tag> {
    times.iter().take(ENTRIES).find_map(|&[day, from, hours]| {
        let end = from.wrapping_add(hours);
        if at.weekday == day {
            (from <= at.hour && at.hour < end).then(|| tag(at.date, from))
        } else {
            let next = if day == 6 { 0 } else { day + 1 };
            (end > 24 && at.weekday == next && at.hour < end - 24).then(|| tag(at.date.wrapping_sub(1), from))
        }
    })
}

/// The `0x0D26` type of the minute, if there is one (`CMap::GetGuildRankNotices`): the start hour at `hour − 1`:30 and :50 (not
/// for an entry that starts at 0), 100 at the start, 101 at `end − 1`:50 and 102 at `end − 1`:59, the last two on the next weekday
/// for an entry past midnight.
pub fn notice(times: &[[u16; 3]], at: Moment) -> Option<u8> {
    times.iter().take(ENTRIES).find_map(|&[day, from, hours]| {
        let end = from.wrapping_add(hours);
        let next = if day == 6 { 0 } else { day + 1 };
        if at.weekday == day {
            if (1..24).contains(&from) && at.hour == from - 1 && matches!(at.minute, 30 | 50) {
                return Some(from as u8);
            }
            if at.hour == from && at.minute == 0 {
                return Some(NOTICE_STARTED);
            }
        }
        // The closing notices come at the last hour before the end.
        let (closing_day, closing_hour) = if end > 24 { (next, end - 25) } else { (day, end.wrapping_sub(1)) };
        if end >= 1 && at.weekday == closing_day && at.hour == closing_hour {
            match at.minute {
                50 => return Some(NOTICE_TEN_MINUTES),
                59 => return Some(NOTICE_ONE_MINUTE),
                _ => {}
            }
        }
        None
    })
}

/// What the minute's step found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Nothing,
    /// The battle has just been found closed; the update is due at this unix time (seconds).
    Closed {
        due: i64,
    },
    /// The update is due now (once).
    Update,
}

/// The battle's state, stepped once per local minute (`CheckGuildRankStatus` and the 30 minutes after).
#[derive(Debug, Default)]
pub struct Battle {
    open: Option<Tag>,
    seen_open: bool,
    due: Option<i64>,
}

impl Battle {
    /// A battle whose update was left due by an earlier run (deviation 4).
    pub fn with_due(due: Option<i64>) -> Self {
        Self { due, ..Self::default() }
    }

    /// The open battle's tag.
    pub fn open(&self) -> Option<Tag> {
        self.open
    }

    /// The unix time the update is waiting for.
    pub fn due(&self) -> Option<i64> {
        self.due
    }

    /// One minute: `open` is `open_tag` of the moment, `now` the unix time in seconds, `delay` the seconds from the close to the
    /// update. A due update comes before a close, so a delay of 0 still takes one more minute.
    pub fn step(&mut self, open: Option<Tag>, now: i64, delay: i64) -> Step {
        self.open = open;
        if self.due.is_some_and(|due| due <= now) {
            self.due = None;
            return Step::Update;
        }
        if open.is_some() {
            self.seen_open = true;
        } else if self.seen_open {
            self.seen_open = false;
            let due = now + delay;
            self.due = Some(due);
            return Step::Closed { due };
        }
        Step::Nothing
    }
}

/// A guild's score (`UpdateGuildPointRank`): its battle points plus the smaller of the Etin returns and 25 per entry together, and
/// half the points; nothing without points.
pub fn score(points: u32, returns: u32, entries: u32) -> u32 {
    if points == 0 {
        return 0;
    }
    let bonus = returns.saturating_add(entries.saturating_mul(25)).min(points / 2);
    points.saturating_add(bonus)
}

/// The places of one country's scored guilds `(guild, score)`: highest score first, equal scores the higher guild id first. The
/// result is `(guild, score, place)` with places from 1; those beyond `PLACES` are included, the caller cuts.
pub fn places(scored: &[(u32, u32)]) -> Vec<(u32, u32, u8)> {
    let mut sorted: Vec<(u32, u32)> = scored.iter().copied().filter(|&(_, s)| s > 0).collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
    sorted.into_iter().enumerate().map(|(i, (guild, s))| (guild, s, u8::try_from(i + 1).unwrap_or(u8::MAX))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUNDAY_NINE: [[u16; 3]; 1] = [[0, 21, 1]];

    fn at(weekday: u16, hour: u16, minute: u16) -> Moment {
        // Day numbers: the weekday decides the date here, as a week starting at 100.
        Moment { weekday, hour, minute, date: 100 + u32::from(weekday) }
    }

    #[test]
    fn the_battle_is_open_by_the_party_rule_with_two_entries() {
        let times = [[0, 21, 1], [3, 14, 2], [5, 10, 1]];
        assert!(open_tag(&times, at(0, 21, 0)).is_some() && open_tag(&times, at(0, 21, 59)).is_some());
        assert!(open_tag(&times, at(0, 20, 59)).is_none() && open_tag(&times, at(0, 22, 0)).is_none());
        assert!(open_tag(&times, at(1, 21, 0)).is_none());
        assert!(open_tag(&times, at(3, 15, 0)).is_some() && open_tag(&times, at(3, 16, 0)).is_none());
        assert!(open_tag(&times, at(5, 10, 0)).is_none(), "only the first two entries are read");
        assert!(open_tag(&[], at(0, 21, 0)).is_none(), "no entries: closed");
        assert_ne!(open_tag(&times, at(0, 21, 0)), open_tag(&times, at(3, 14, 0)));
        // The tag is the same all through one battle and differs by week.
        assert_eq!(open_tag(&times, at(0, 21, 0)), open_tag(&times, at(0, 21, 59)));
        let next_week = Moment { date: 107, ..at(0, 21, 0) };
        assert_ne!(open_tag(&times, at(0, 21, 0)), open_tag(&times, next_week));
    }

    #[test]
    fn a_battle_past_midnight_runs_into_the_next_weekday_with_one_tag() {
        // Saturday 23:00 for 3 hours: until 02:00 on Sunday.
        let times = [[6, 23, 3]];
        let saturday = Moment { weekday: 6, hour: 23, minute: 30, date: 106 };
        let sunday = Moment { weekday: 0, hour: 1, minute: 59, date: 107 };
        assert!(open_tag(&times, saturday).is_some());
        assert_eq!(open_tag(&times, saturday), open_tag(&times, sunday));
        assert!(open_tag(&times, Moment { hour: 2, ..sunday }).is_none());
        assert!(open_tag(&times, Moment { weekday: 6, hour: 22, minute: 0, date: 106 }).is_none());
    }

    #[test]
    fn notices_come_at_their_minutes() {
        let n = |w, h, m| notice(&SUNDAY_NINE, at(w, h, m));
        assert_eq!((n(0, 20, 30), n(0, 20, 50)), (Some(21), Some(21)));
        assert_eq!((n(0, 20, 0), n(0, 20, 31), n(0, 19, 30)), (None, None, None));
        assert_eq!((n(0, 21, 0), n(0, 21, 1)), (Some(NOTICE_STARTED), None));
        assert_eq!((n(0, 21, 50), n(0, 21, 59), n(0, 21, 30)), (Some(NOTICE_TEN_MINUTES), Some(NOTICE_ONE_MINUTE), None));
        assert_eq!((n(1, 20, 30), n(1, 21, 0), n(1, 21, 50)), (None, None, None), "other weekdays");
    }

    #[test]
    fn an_entry_at_hour_zero_has_no_early_notice_and_one_past_midnight_closes_the_next_day() {
        let midnight = [[2, 0, 1]];
        assert_eq!(notice(&midnight, at(2, 23, 30)), None);
        assert_eq!(notice(&midnight, at(2, 0, 0)), Some(NOTICE_STARTED));
        assert_eq!(notice(&midnight, at(2, 0, 50)), Some(NOTICE_TEN_MINUTES));
        let late = [[6, 23, 3]];
        assert_eq!(notice(&late, at(6, 22, 30)), Some(23));
        assert_eq!(notice(&late, at(6, 23, 0)), Some(NOTICE_STARTED));
        assert_eq!(notice(&late, at(6, 23, 50)), None, "the closing notices are on Sunday");
        assert_eq!(notice(&late, at(0, 1, 50)), Some(NOTICE_TEN_MINUTES));
        assert_eq!(notice(&late, at(0, 1, 59)), Some(NOTICE_ONE_MINUTE));
        assert_eq!(notice(&late, at(0, 2, 50)), None);
        // Ending exactly at midnight: the same weekday, 23:50 and 23:59.
        let till_midnight = [[3, 23, 1]];
        assert_eq!(notice(&till_midnight, at(3, 23, 50)), Some(NOTICE_TEN_MINUTES));
        assert_eq!(notice(&till_midnight, at(3, 23, 59)), Some(NOTICE_ONE_MINUTE));
    }

    #[test]
    fn the_close_sets_the_update_and_it_runs_once() {
        let tag = open_tag(&SUNDAY_NINE, at(0, 21, 0));
        let mut battle = Battle::default();
        assert_eq!(battle.step(None, 0, 1800), Step::Nothing, "never open: no update");
        assert_eq!(battle.step(tag, 60, 1800), Step::Nothing);
        assert_eq!(battle.open(), tag);
        assert_eq!(battle.step(tag, 120, 1800), Step::Nothing);
        assert_eq!(battle.step(None, 180, 1800), Step::Closed { due: 1980 });
        assert_eq!((battle.open(), battle.due()), (None, Some(1980)));
        assert_eq!(battle.step(None, 240, 1800), Step::Nothing);
        assert_eq!(battle.step(None, 1980, 1800), Step::Update);
        assert_eq!((battle.step(None, 2040, 1800), battle.due()), (Step::Nothing, None));
    }

    #[test]
    fn an_update_left_due_runs_at_the_first_minute_after_it() {
        let mut battle = Battle::with_due(Some(500));
        assert_eq!(battle.step(None, 400, 1800), Step::Nothing);
        assert_eq!(battle.step(None, 600, 1800), Step::Update);
        assert_eq!(battle.step(None, 660, 1800), Step::Nothing);
    }

    #[test]
    fn a_zero_delay_takes_one_more_minute() {
        let tag = open_tag(&SUNDAY_NINE, at(0, 21, 0));
        let mut battle = Battle::default();
        battle.step(tag, 0, 0);
        assert_eq!(battle.step(None, 60, 0), Step::Closed { due: 60 });
        assert_eq!(battle.step(None, 120, 0), Step::Update);
    }

    #[test]
    fn the_score_adds_a_bonus_of_up_to_half_the_points() {
        assert_eq!(score(100, 3, 4), 150);
        assert_eq!(score(100, 3, 1), 128);
        assert_eq!(score(100, 0, 0), 100);
        assert_eq!(score(101, 500, 0), 101 + 50, "half is an integer half");
        assert_eq!(score(0, 10, 10), 0, "no points, no score");
        assert_eq!(score(u32::MAX, u32::MAX, u32::MAX), u32::MAX);
    }

    #[test]
    fn places_go_by_score_then_by_the_higher_guild_id() {
        let ranked = places(&[(1, 50), (2, 80), (3, 50), (4, 0), (5, 10)]);
        assert_eq!(ranked, vec![(2, 80, 1), (3, 50, 2), (1, 50, 3), (5, 10, 4)]);
        assert!(places(&[]).is_empty() && places(&[(1, 0)]).is_empty());
        let many: Vec<(u32, u32)> = (1..=40).map(|g| (g, g * 10)).collect();
        let ranked = places(&many);
        assert_eq!((ranked[0], ranked[29], ranked[30], ranked[39]), ((40, 400, 1), (11, 110, 30), (10, 100, 31), (1, 10, 40)));
    }
}
