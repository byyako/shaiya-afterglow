//! The ids an extension may own and the client's ceilings (`docs/plans/ext-1-foundation.md` section 5.6). Every number has its source;
//! client research entry 11 (a) will replace or extend them.

/// Item types: `Item.SData` has 100 groups (`docs/client/sdata-generation.md` section 1; verified). Whether the client takes more: entry 11 (a).
pub const ITEM_TYPES: i64 = 100;
/// The skill an item gives is one byte in the client (`docs/client/research-gambling-kill-boost.md`, "what adding dedicated content takes").
pub const ITEM_RANGE_MAX: i64 = 255;
/// `Skill.SData` holds 326 skills; growing the table is refused by `client_patch.py` (check 6). The loader takes any count (entry 7, inferred).
pub const SKILLS: i64 = 326;
/// Players' skill levels; level 100 is the monsters' (`NpcSkill.SData`, not generated).
pub const SKILL_LEVELS: i64 = 3;

/// The records of each of the 100 groups of the US patch 108 `Item.SData` (the dense arrays of the file; 0 for the 26 groups without
/// records, so nothing is placed there; 74 groups, 16,045 records). A test compares it with `client.items` when that schema exists.
pub const ITEM_GROUP_RECORDS: [u16; 100] = [
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 255,
    255, 30, 255, 255, 255, 255, 255, 255, 255, 0, 0, 255, 255, 0, 32, 4, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 54, 54, 244, 54, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 54, 54, 244, 54, 54, 0, 0, 0, 0, 0,
    0, 0, 6, 8, 0, 0, 0, 0, 255,
];

/// One declared range of ids: items `major` = type, `low..=high` type ids; skills `major` = skill id, levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdRange {
    pub major: i64,
    pub low: i64,
    pub high: i64,
}

impl IdRange {
    pub fn contains(&self, major: i64, minor: i64) -> bool {
        self.major == major && (self.low..=self.high).contains(&minor)
    }

    pub fn overlaps(&self, other: &IdRange) -> bool {
        self.major == other.major && self.low <= other.high && other.low <= self.high
    }

    pub fn text(&self) -> String {
        if self.low == self.high { format!("{}/{}", self.major, self.low) } else { format!("{}/{}-{}", self.major, self.low, self.high) }
    }
}

/// `"100/212-214"`, `"101/5"` (items), or `"200/1-3"`, `"201/2"`, `"201"` (every level; skills only).
pub fn parse_range(text: &str, skill: bool) -> Result<IdRange, String> {
    let bad = || {
        format!(
            "{text:?} is not an id range ({})",
            if skill { "skill id, \"id/level\" or \"id/low-high\"" } else { "\"type/typeid\" or \"type/low-high\"" }
        )
    };
    let number = |t: &str| t.trim().parse::<i64>().map_err(|_| bad());
    let (major, minor) = match text.split_once('/') {
        Some((major, minor)) => (number(major)?, Some(minor)),
        None if skill => (number(text)?, None),
        None => return Err(bad()),
    };
    let (low, high) = match minor {
        None => (1, SKILL_LEVELS),
        Some(minor) => match minor.split_once('-') {
            Some((low, high)) => (number(low)?, number(high)?),
            None => (number(minor)?, number(minor)?),
        },
    };
    if low > high {
        return Err(format!("{text:?}: the range runs backwards"));
    }
    let range = IdRange { major, low, high };
    check_ceilings(&range, skill)?;
    Ok(range)
}

/// A range inside the ceilings; the message says which and why.
pub fn check_ceilings(range: &IdRange, skill: bool) -> Result<(), String> {
    let IdRange { major, low, high } = *range;
    if skill {
        if !(1..=SKILLS).contains(&major) {
            return Err(format!(
                "skill {major}: Skill.SData holds skills 1 to {SKILLS} (growing it is refused, docs/plans/ext-1-foundation.md 5.6)"
            ));
        }
        if low < 1 || high > SKILL_LEVELS {
            return Err(format!("skill {major}: levels {low}-{high}: player levels are 1 to {SKILL_LEVELS} (level 100 is the monsters')"));
        }
    } else {
        if !(1..=ITEM_TYPES).contains(&major) {
            return Err(format!(
                "item type {major}: Item.SData has types 1 to {ITEM_TYPES} (docs/plans/ext-1-foundation.md 5.6; client research entry 11 a)"
            ));
        }
        let records = i64::from(ITEM_GROUP_RECORDS[major as usize - 1]);
        if records == 0 {
            return Err(format!("item type {major} has no records in Item.SData: nothing is placed there"));
        }
        if low < 1 || high > records {
            return Err(format!(
                "item type {major}: type ids {low}-{high}, but its group of Item.SData holds 1 to {records} (growing a group is refused)"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_group_table_is_the_files() {
        assert_eq!(ITEM_GROUP_RECORDS.iter().filter(|&&n| n > 0).count(), 74);
        assert_eq!(ITEM_GROUP_RECORDS.iter().map(|&n| u32::from(n)).sum::<u32>(), 16045);
    }

    #[test]
    fn ranges_parse_and_stay_inside_the_ceilings() {
        assert_eq!(parse_range("100/212-214", false).unwrap(), IdRange { major: 100, low: 212, high: 214 });
        assert_eq!(parse_range("100/5", false).unwrap(), IdRange { major: 100, low: 5, high: 5 });
        assert_eq!(parse_range("200/1-3", true).unwrap(), IdRange { major: 200, low: 1, high: 3 });
        assert_eq!(parse_range("201", true).unwrap(), IdRange { major: 201, low: 1, high: 3 });
        assert_eq!(parse_range("201/2", true).unwrap(), IdRange { major: 201, low: 2, high: 2 });
        for (text, skill, needle) in [
            ("101/5", false, "Item.SData has types 1 to 100"),
            ("0/5", false, "types 1 to 100"),
            ("26/1", false, "has no records"),
            ("43/5", false, "holds 1 to 4"),
            ("100/256", false, "holds 1 to 255"),
            ("100/0-3", false, "holds 1 to 255"),
            ("327", true, "skills 1 to 326"),
            ("200/4", true, "levels 4-4"),
            ("200/100", true, "level 100 is the monsters'"),
            ("200/3-1", true, "backwards"),
            ("100", false, "not an id range"),
            ("x/1", false, "not an id range"),
        ] {
            let error = parse_range(text, skill).unwrap_err();
            assert!(error.contains(needle), "{text}: {error}");
        }
    }

    #[test]
    fn overlap_and_contains() {
        let a = IdRange { major: 100, low: 212, high: 214 };
        assert!(a.contains(100, 213) && !a.contains(100, 215) && !a.contains(101, 213));
        assert!(a.overlaps(&IdRange { major: 100, low: 214, high: 220 }) && !a.overlaps(&IdRange { major: 100, low: 215, high: 220 }));
        assert_eq!(a.text(), "100/212-214");
    }
}
