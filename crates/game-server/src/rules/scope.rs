//! The scope of a rule (`docs/plans/ext-2-rules.md` section 3): where and for whom it holds, compiled to plain data (section 8): sorted
//! ranges with a `not` flag, an optional faction and a 168-bit hour-of-week mask. Nothing here touches strings or JSON when asked.

use std::sync::atomic::{AtomicU16, Ordering};

use serde_json::{Map, Value};

use super::{RuleError, key_err};

/// What a rule tests of a player: the level, the kill count the client shows (`kills[0]`) and the faction (country 0 Light, 1 Fury).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Who {
    pub level: u16,
    pub kills: u32,
    pub country: u8,
}

/// Which of the two players of a two-player point the player keys of the scope are tested on (section 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhoMode {
    Either,
    Both,
    Actor,
    Target,
}

/// Sorted, merged, disjoint ranges of `u32`, or everything but those.
#[derive(Clone, Debug)]
pub(super) struct Ranges {
    list: Vec<(u32, u32)>,
    not: bool,
}

impl Ranges {
    fn new(mut list: Vec<(u32, u32)>, not: bool) -> Self {
        list.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(list.len());
        for (low, high) in list {
            match merged.last_mut() {
                Some(last) if low <= last.1.saturating_add(1) => last.1 = last.1.max(high),
                _ => merged.push((low, high)),
            }
        }
        Self { list: merged, not }
    }

    fn contains(&self, n: u32) -> bool {
        let at = self.list.partition_point(|&(_, high)| high < n);
        (at < self.list.len() && self.list[at].0 <= n) != self.not
    }
}

/// The hour of the week, 0 to 167, Sunday 0:00 first, in the server's local time (section 8, point 4); `u16::MAX` until first read.
static HOUR_OF_WEEK: AtomicU16 = AtomicU16::new(u16::MAX);

fn now_hour_of_week() -> u16 {
    use chrono::{Datelike, Timelike};
    let now = chrono::Local::now();
    (now.weekday().num_days_from_sunday() * 24 + now.hour()) as u16
}

/// The hour of the week now: refreshed by [`set_hour_of_week`] (`main` every 30 seconds); read from the clock the first time.
pub fn hour_of_week() -> u16 {
    match HOUR_OF_WEEK.load(Ordering::Relaxed) {
        u16::MAX => {
            let now = now_hour_of_week();
            HOUR_OF_WEEK.store(now, Ordering::Relaxed);
            now
        }
        hour => hour,
    }
}

/// Sets the hour of the week (0 to 167); tests set it themselves.
pub fn set_hour_of_week(hour: u16) {
    HOUR_OF_WEEK.store(hour % 168, Ordering::Relaxed);
}

/// Sets the hour of the week from the clock.
pub fn refresh_hour_of_week() {
    set_hour_of_week(now_hour_of_week());
}

/// The tests that set the clock take this lock: the hour is one value for the whole test binary.
#[cfg(test)]
pub(crate) static CLOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Debug)]
pub(super) struct Scope {
    maps: Option<Ranges>,
    level: Option<Ranges>,
    kills: Option<Ranges>,
    faction: Option<u8>,
    time: Option<[u64; 3]>,
    who: WhoMode,
}

impl Scope {
    fn time_ok(&self) -> bool {
        match &self.time {
            None => true,
            Some(mask) => {
                let hour = usize::from(hour_of_week());
                (mask[hour / 64] >> (hour % 64)) & 1 == 1
            }
        }
    }

    fn player_ok(&self, w: &Who) -> bool {
        self.level.as_ref().is_none_or(|r| r.contains(u32::from(w.level)))
            && self.kills.as_ref().is_none_or(|r| r.contains(w.kills))
            && self.faction.is_none_or(|f| f == w.country)
    }

    fn place_ok(&self, map: u16) -> bool {
        self.maps.as_ref().is_none_or(|r| r.contains(u32::from(map))) && self.time_ok()
    }

    /// Whether the scope holds for a one-player point.
    pub fn one(&self, map: u16, a: &Who) -> bool {
        self.place_ok(map) && self.player_ok(a)
    }

    /// Whether the scope holds for a two-player point: `a` the actor (the attacker, the requester), `b` the target.
    pub fn two(&self, map: u16, a: &Who, b: &Who) -> bool {
        self.place_ok(map)
            && match self.who {
                WhoMode::Either => self.player_ok(a) || self.player_ok(b),
                WhoMode::Both => self.player_ok(a) && self.player_ok(b),
                WhoMode::Actor => self.player_ok(a),
                WhoMode::Target => self.player_ok(b),
            }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The parser
// ---------------------------------------------------------------------------------------------------------------------------------

/// `{ not = X }` is the negation of `X`; a table with any other key, or `not` inside `not`, is an error.
fn negated(v: &Value) -> Result<(&Value, bool), String> {
    match v {
        Value::Object(m) => match (m.len(), m.get("not")) {
            (1, Some(inner)) if !inner.is_object() => Ok((inner, true)),
            (1, Some(_)) => Err("`not` cannot be nested".into()),
            _ => Err("a table here holds only `not`".into()),
        },
        other => Ok((other, false)),
    }
}

/// One number within `low..=high`.
fn single(n: &Value, low: u32, high: u32) -> Result<u32, String> {
    match (n.as_u64(), n.as_i64()) {
        (Some(n), _) if n >= u64::from(low) && n <= u64::from(high) => Ok(n as u32),
        (Some(n), _) => Err(format!("{n} is not {low} to {high}")),
        (None, Some(i)) => Err(format!("{i} is not {low} to {high}")),
        (None, None) => Err(format!("{n} is not a whole number")),
    }
}

fn number_text(text: &str, low: u32, high: u32) -> Result<u32, String> {
    let n: u64 = text.trim().parse().map_err(|_| format!("{:?} is not a number or a range \"a-b\"", text.trim()))?;
    if n < u64::from(low) || n > u64::from(high) {
        return Err(format!("{n} is not {low} to {high}"));
    }
    Ok(n as u32)
}

/// A list of numbers and ranges (`30`, `"1-15"`, `"60-"`, `"-30"`) inside `low..=high`. `list_only`: a bare number or string is refused.
fn ranges_of(v: &Value, low: u32, high: u32, list_only: bool) -> Result<Vec<(u32, u32)>, String> {
    let items: Vec<&Value> = match v {
        Value::Array(items) => items.iter().collect(),
        other if list_only => return Err(format!("needs a list, e.g. [{}] (not {other})", low.max(1))),
        other => vec![other],
    };
    if items.is_empty() {
        return Err("an empty list matches nothing".into());
    }
    items
        .into_iter()
        .map(|item| match item {
            Value::Number(_) => single(item, low, high).map(|n| (n, n)),
            Value::String(text) => match text.trim().split_once('-') {
                None => number_text(text, low, high).map(|n| (n, n)),
                Some((a, b)) => {
                    if a.trim().is_empty() && b.trim().is_empty() {
                        return Err(format!("{:?} needs a number on one side", text.trim()));
                    }
                    let a = if a.trim().is_empty() { low } else { number_text(a, low, high)? };
                    let b = if b.trim().is_empty() { high } else { number_text(b, low, high)? };
                    if a > b {
                        return Err(format!("{:?}: the low end {a} is above the high end {b}", text.trim()));
                    }
                    Ok((a, b))
                }
            },
            other => Err(format!("a number or a range \"a-b\", not {other}")),
        })
        .collect()
}

/// `key` of `obj` as ranges (taking it out), `{ not = … }` allowed.
fn take_ranges(obj: &mut Map<String, Value>, key: &str, low: u32, high: u32, list_only: bool) -> Result<Option<Ranges>, RuleError> {
    let Some(value) = obj.remove(key) else { return Ok(None) };
    let (inner, not) = negated(&value).map_err(|m| key_err(key, m))?;
    let list = ranges_of(inner, low, high, list_only).map_err(|m| key_err(key, m))?;
    Ok(Some(Ranges::new(list, not)))
}

/// The scope keys of `obj`, taken out of it. `who`: the default for a two-player rule, `None` where `who` is not a key of the rule;
/// `kills_scope`: false where `kills` is the rule's own key (`pvp_gain`).
pub(super) fn parse(obj: &mut Map<String, Value>, who: Option<WhoMode>, kills_scope: bool) -> Result<Scope, RuleError> {
    let maps = take_ranges(obj, "maps", 0, 65535, true)?;
    let level = take_ranges(obj, "level", 1, 255, false)?;
    let kills = if kills_scope { take_ranges(obj, "kills", 0, u32::MAX, false)? } else { None };
    let faction = match obj.remove("faction") {
        None => None,
        Some(Value::String(f)) if f == "light" => Some(0),
        Some(Value::String(f)) if f == "fury" => Some(1),
        Some(other) => return Err(key_err("faction", format!("{other} is not \"light\" or \"fury\""))),
    };
    let days = take_ranges(obj, "days", 0, 6, true)?;
    let hours = take_ranges(obj, "hours", 0, 23, false)?;
    let time = (days.is_some() || hours.is_some()).then(|| {
        let mut mask = [0u64; 3];
        for hour_of_week in 0..168u32 {
            let (day, hour) = (hour_of_week / 24, hour_of_week % 24);
            if days.as_ref().is_none_or(|r| r.contains(day)) && hours.as_ref().is_none_or(|r| r.contains(hour)) {
                mask[(hour_of_week / 64) as usize] |= 1 << (hour_of_week % 64);
            }
        }
        mask
    });
    let mode = match (obj.remove("who"), who) {
        (None, default) => default.unwrap_or(WhoMode::Either),
        (Some(_), None) => return Err(RuleError::unknown("who")),
        (Some(Value::String(w)), Some(_)) => match w.as_str() {
            "either" => WhoMode::Either,
            "both" => WhoMode::Both,
            "actor" => WhoMode::Actor,
            "target" => WhoMode::Target,
            other => return Err(key_err("who", format!("{other:?} is not \"either\", \"both\", \"actor\" or \"target\""))),
        },
        (Some(other), Some(_)) => return Err(key_err("who", format!("{other} is not \"either\", \"both\", \"actor\" or \"target\""))),
    };
    Ok(Scope { maps, level, kills, faction, time, who: mode })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn scope(v: Value, who: Option<WhoMode>) -> Result<Scope, String> {
        let Value::Object(mut obj) = v else { panic!("an object") };
        let s = parse(&mut obj, who, true).map_err(|e| e.msg.clone())?;
        assert!(obj.is_empty(), "keys left over: {obj:?}");
        Ok(s)
    }

    fn who(level: u16, kills: u32, country: u8) -> Who {
        Who { level, kills, country }
    }

    const ANY: Who = Who { level: 30, kills: 0, country: 0 };

    #[test]
    fn a_scope_without_keys_matches_everything() {
        let s = scope(json!({}), None).unwrap();
        assert!(s.one(0, &ANY) && s.one(65535, &who(255, u32::MAX, 1)));
    }

    #[test]
    fn maps_are_a_list_of_ids_and_ranges() {
        let s = scope(json!({ "maps": [35, 36, "40-42"] }), None).unwrap();
        for map in [35, 36, 40, 41, 42] {
            assert!(s.one(map, &ANY), "{map}");
        }
        for map in [0, 34, 37, 39, 43, 65535] {
            assert!(!s.one(map, &ANY), "{map}");
        }
        let edge = scope(json!({ "maps": [0, 65535] }), None).unwrap();
        assert!(edge.one(0, &ANY) && edge.one(65535, &ANY) && !edge.one(1, &ANY));
        assert!(scope(json!({ "maps": [65536] }), None).unwrap_err().contains("65536 is not 0 to 65535"));
        assert!(scope(json!({ "maps": 35 }), None).unwrap_err().contains("needs a list"));
        assert!(scope(json!({ "maps": [] }), None).unwrap_err().contains("empty list"));
        assert!(scope(json!({ "maps": ["43-40"] }), None).unwrap_err().contains("low end 43 is above the high end 40"));
        assert!(scope(json!({ "maps": [-1] }), None).is_err());
    }

    #[test]
    fn overlapping_ranges_merge() {
        let s = scope(json!({ "maps": ["1-5", "4-9", 10, "20-25"] }), None).unwrap();
        assert!((1..=10).all(|m| s.one(m, &ANY)) && !s.one(11, &ANY) && s.one(20, &ANY) && s.one(25, &ANY) && !s.one(26, &ANY));
    }

    #[test]
    fn levels_take_a_range_open_ends_a_number_and_a_list() {
        let at = |v: Value, level: u16| scope(json!({ "level": v }), None).unwrap().one(1, &who(level, 0, 0));
        assert!(at(json!("1-15"), 1) && at(json!("1-15"), 15) && !at(json!("1-15"), 16));
        assert!(at(json!("60-"), 60) && at(json!("60-"), 255) && !at(json!("60-"), 59));
        assert!(at(json!("-30"), 1) && at(json!("-30"), 30) && !at(json!("-30"), 31));
        assert!(at(json!(30), 30) && !at(json!(30), 29) && !at(json!(30), 31));
        assert!(at(json!("30"), 30) && !at(json!("30"), 31));
        assert!(at(json!([5, "10-12", "60-"]), 5) && at(json!([5, "10-12", "60-"]), 11) && at(json!([5, "10-12", "60-"]), 200));
        assert!(!at(json!([5, "10-12", "60-"]), 6) && !at(json!([5, "10-12", "60-"]), 13));
        let error = |v: Value| scope(json!({ "level": v }), None).unwrap_err();
        assert!(error(json!(0)).contains("0 is not 1 to 255"));
        assert!(error(json!("0-5")).contains("0 is not 1 to 255"));
        assert!(error(json!(256)).contains("256 is not 1 to 255"));
        assert!(error(json!("20-10")).contains("above the high end"));
        assert!(error(json!("-")).contains("one side"));
        assert!(error(json!("x")).contains("not a number or a range"));
        assert!(error(json!(1.5)).contains("not a whole number"));
        assert!(error(json!(true)).contains("a number or a range"));
    }

    #[test]
    fn kills_are_tested_on_the_kill_count() {
        let s = scope(json!({ "kills": "100-" }), None).unwrap();
        assert!(s.one(1, &who(30, 100, 0)) && !s.one(1, &who(30, 99, 0)) && s.one(1, &who(30, u32::MAX, 0)));
        let zero = scope(json!({ "kills": 0 }), None).unwrap();
        assert!(zero.one(1, &who(30, 0, 0)) && !zero.one(1, &who(30, 1, 0)));
        assert!(scope(json!({ "kills": 4294967296u64 }), None).unwrap_err().contains("is not 0 to 4294967295"));
    }

    #[test]
    fn faction_is_light_or_fury() {
        let light = scope(json!({ "faction": "light" }), None).unwrap();
        let fury = scope(json!({ "faction": "fury" }), None).unwrap();
        assert!(light.one(1, &who(30, 0, 0)) && !light.one(1, &who(30, 0, 1)));
        assert!(fury.one(1, &who(30, 0, 1)) && !fury.one(1, &who(30, 0, 0)));
        assert!(scope(json!({ "faction": "dark" }), None).unwrap_err().contains("not \"light\" or \"fury\""));
        assert!(scope(json!({ "faction": 0 }), None).is_err());
    }

    #[test]
    fn not_turns_each_key_around() {
        let maps = scope(json!({ "maps": { "not": [42] } }), None).unwrap();
        assert!(maps.one(1, &ANY) && !maps.one(42, &ANY));
        let level = scope(json!({ "level": { "not": "1-15" } }), None).unwrap();
        assert!(!level.one(1, &who(15, 0, 0)) && level.one(1, &who(16, 0, 0)));
        let kills = scope(json!({ "kills": { "not": "10-" } }), None).unwrap();
        assert!(kills.one(1, &who(30, 9, 0)) && !kills.one(1, &who(30, 10, 0)));
        let days = scope(json!({ "days": { "not": [0, 6] } }), None).unwrap();
        let hours = scope(json!({ "hours": { "not": "18-22" } }), None).unwrap();
        let _guard = CLOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_hour_of_week(24 * 3 + 12);
        assert!(days.one(1, &ANY) && hours.one(1, &ANY));
        set_hour_of_week(6);
        assert!(!days.one(1, &ANY) && hours.one(1, &ANY));
        set_hour_of_week(24 * 2 + 20);
        assert!(days.one(1, &ANY) && !hours.one(1, &ANY));
        for bad in [json!({ "not": [1], "x": 1 }), json!({ "x": [1] }), json!({ "not": { "not": [1] } })] {
            assert!(scope(json!({ "maps": bad }), None).is_err());
        }
        assert!(scope(json!({ "maps": { "not": { "not": [1] } } }), None).unwrap_err().contains("cannot be nested"));
        assert!(scope(json!({ "faction": { "not": "light" } }), None).is_err(), "faction has no not");
    }

    #[test]
    fn days_and_hours_together_mean_those_days_at_those_hours() {
        let s = scope(json!({ "days": [1, 2, 3, 4, 5], "hours": "18-22" }), None).unwrap();
        let _guard = CLOCK.lock().unwrap_or_else(|e| e.into_inner());
        let at = |day: u16, hour: u16| {
            set_hour_of_week(day * 24 + hour);
            s.one(1, &ANY)
        };
        assert!(at(1, 18) && at(1, 22) && at(5, 20) && at(3, 19));
        assert!(!at(1, 17) && !at(1, 23) && !at(0, 20) && !at(6, 20) && !at(3, 0));
        let hour = scope(json!({ "hours": 0 }), None).unwrap();
        assert!(hour.time_ok_at(0) && !hour.time_ok_at(1) && hour.time_ok_at(24) && hour.time_ok_at(167 - 23));
        let list = scope(json!({ "hours": [3, "20-23"] }), None).unwrap();
        assert!(list.time_ok_at(3) && list.time_ok_at(23) && !list.time_ok_at(4) && list.time_ok_at(167));
        let sunday = scope(json!({ "days": [0] }), None).unwrap();
        assert!(sunday.time_ok_at(0) && sunday.time_ok_at(23) && !sunday.time_ok_at(24));
        assert!(scope(json!({ "days": [7] }), None).unwrap_err().contains("7 is not 0 to 6"));
        assert!(scope(json!({ "hours": "24" }), None).unwrap_err().contains("24 is not 0 to 23"));
        assert!(scope(json!({ "hours": "22-18" }), None).unwrap_err().contains("above the high end"));
        assert!(scope(json!({ "days": 1 }), None).unwrap_err().contains("needs a list"));
    }

    impl Scope {
        fn time_ok_at(&self, hour: u16) -> bool {
            set_hour_of_week(hour);
            self.time_ok()
        }
    }

    #[test]
    fn keys_are_anded_and_entries_ored() {
        let s = scope(json!({ "maps": [1, 2], "level": ["1-10", "50-"], "faction": "fury" }), None).unwrap();
        assert!(s.one(1, &who(5, 0, 1)) && s.one(2, &who(60, 0, 1)));
        assert!(!s.one(3, &who(5, 0, 1)) && !s.one(1, &who(30, 0, 1)) && !s.one(1, &who(5, 0, 0)));
    }

    #[test]
    fn who_picks_the_players_the_keys_are_tested_on() {
        let low = who(10, 0, 0);
        let high = who(60, 0, 1);
        let at = |w: &str| scope(json!({ "level": "1-15", "who": w }), Some(WhoMode::Either)).unwrap();
        assert!(at("either").two(1, &low, &high) && at("either").two(1, &high, &low) && !at("either").two(1, &high, &high));
        assert!(at("both").two(1, &low, &low) && !at("both").two(1, &low, &high) && !at("both").two(1, &high, &low));
        assert!(at("actor").two(1, &low, &high) && !at("actor").two(1, &high, &low));
        assert!(at("target").two(1, &high, &low) && !at("target").two(1, &low, &high));
        // The default is the point's.
        let default = |mode| scope(json!({ "level": "1-15" }), Some(mode)).unwrap();
        assert!(default(WhoMode::Either).two(1, &high, &low) && !default(WhoMode::Both).two(1, &high, &low));
        // The player keys are tested together on one player: level and faction of the same side.
        let both = scope(json!({ "level": "1-15", "faction": "fury", "who": "either" }), Some(WhoMode::Either)).unwrap();
        assert!(!both.two(1, &who(10, 0, 0), &who(60, 0, 1)), "one side low, the other fury: neither side is both");
        assert!(both.two(1, &who(10, 0, 1), &who(60, 0, 0)));
        // A map or time key is not about a player.
        let on_map = scope(json!({ "maps": [1], "who": "both" }), Some(WhoMode::Either)).unwrap();
        assert!(on_map.two(1, &low, &high) && !on_map.two(2, &low, &high));
        assert!(scope(json!({ "who": "other" }), Some(WhoMode::Either)).unwrap_err().contains("not \"either\""));
        assert!(scope(json!({ "who": 1 }), Some(WhoMode::Either)).is_err());
    }

    #[test]
    fn who_is_an_unknown_key_on_a_one_player_point() {
        let Value::Object(mut obj) = json!({ "who": "both" }) else { unreachable!() };
        let error = parse(&mut obj, None, true).unwrap_err();
        assert_eq!(error.msg, "unknown key \"who\"");
    }
}
