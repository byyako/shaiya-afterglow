//! The tables an extension may touch in chunk 1 (`docs/plans/ext-1-foundation.md` section 5.5): one line per table. A later chunk adds a
//! line, not a mechanism. Everything else (monsters, NPCs, shops, quests, the item mall, spawns, portals, obelisks, bosses) is refused.

use crate::registry::Side;

#[derive(Clone, Copy, Debug)]
pub struct ExtTable {
    /// The table as `content.ext_rows.tbl` names it: `defs.items`. The schema part is the side's, whatever the scratch name.
    pub tbl: &'static str,
    pub side: Side,
    pub table: &'static str,
    /// The key columns, in the order of the key text (`100212`, `200/1`, `2133/4`).
    pub key_cols: &'static [&'static str],
}

pub static TABLES: &[ExtTable] = &[
    ExtTable { tbl: "defs.items", side: Side::Defs, table: "items", key_cols: &["itemid"] },
    ExtTable { tbl: "defs.skills", side: Side::Defs, table: "skills", key_cols: &["skillid", "skilllevel"] },
    ExtTable { tbl: "defs.mob_items", side: Side::Defs, table: "mob_items", key_cols: &["mobid", "itemorder"] },
];

/// The extendable table named `defs.items` (or just `items`).
pub fn find(name: &str) -> Option<&'static ExtTable> {
    let bare = name.rsplit('.').next().unwrap_or(name);
    TABLES.iter().find(|t| t.tbl == name || t.table == bare)
}

/// A key text (`200/1`) as the numbers of the key columns.
pub fn parse_key(text: &str) -> Result<Vec<i64>, String> {
    text.split('/').map(|p| p.trim().parse::<i64>().map_err(|_| format!("{text:?} is not a key (numbers separated by /)"))).collect()
}
