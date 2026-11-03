//! What holds or points at an extension's rows (`docs/plans/ext-1-foundation.md` section 5.7): the holder check (players' items and skills),
//! the reference check (other data that names the items and skills), the forced deletion of holders, and the "players online" guard.
//! The tables of the game (`character_items` …) are named without a schema, as the servers name them.

use anyhow::Result;
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, PgConnection};

use crate::registry::Schemas;

fn sql(text: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(text)
}

/// A game server counts as running when its heartbeat is this recent (twice the default heartbeat of 5 s).
pub const HEARTBEAT_WINDOW_SECS: i32 = 10;

fn item_text(id: i32) -> String {
    format!("{}/{}", id / 1000, id % 1000)
}

/// Holders of the items (`ids` are item ids, type × 1000 + type id) and of the skill levels: one line per holder.
pub async fn holders(conn: &mut PgConnection, items: &[i32], skills: &[(i32, i32)]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if !items.is_empty() {
        let rows: Vec<(String, i32, i16, i16, i16, i32)> = sqlx::query_as(
            "SELECT c.name::text, c.id, i.bag, i.slot, i.count, i.item_id FROM character_items i JOIN characters c ON c.id = i.character_id
             WHERE i.item_id = ANY($1) ORDER BY c.name, i.bag, i.slot",
        )
        .bind(items)
        .fetch_all(&mut *conn)
        .await?;
        for (name, id, bag, slot, count, item) in rows {
            let place = if bag == 0 { format!("worn, slot {slot}") } else { format!("in bag {bag}, slot {slot}") };
            out.push(format!("character {name} ({id}): {count} × {} {place}", item_text(item)));
        }
        let rows: Vec<(String, i32, i16, i16, i16, i32)> = sqlx::query_as(
            "SELECT a.username::text, b.user_id, b.server_id, b.slot, b.count, b.item_id FROM bank_items b JOIN accounts a ON a.id = b.user_id
             WHERE b.item_id = ANY($1) ORDER BY a.username, b.slot",
        )
        .bind(items)
        .fetch_all(&mut *conn)
        .await?;
        for (name, id, server, slot, count, item) in rows {
            out.push(format!("account {name} ({id}): {count} × {} in the bank of server {server}, slot {slot}", item_text(item)));
        }
        let rows: Vec<(String, i32, i16, i16, i32)> = sqlx::query_as(
            "SELECT g.name::text, b.guild_id, b.slot, b.count, b.item_id FROM guild_bank_items b JOIN guilds g ON g.id = b.guild_id
             WHERE b.item_id = ANY($1) ORDER BY g.name, b.slot",
        )
        .bind(items)
        .fetch_all(&mut *conn)
        .await?;
        for (name, id, slot, count, item) in rows {
            out.push(format!("guild {name} ({id}): {count} × {} in the guild bank, slot {slot}", item_text(item)));
        }
        let rows: Vec<(String, i32, i32, i16, i32)> = sqlx::query_as(
            "SELECT c.name::text, c.id, m.id, m.count, m.item_id FROM market_auctions m JOIN characters c ON c.id = m.seller_id
             WHERE m.item_id = ANY($1) ORDER BY m.id",
        )
        .bind(items)
        .fetch_all(&mut *conn)
        .await?;
        for (name, id, auction, count, item) in rows {
            out.push(format!("character {name} ({id}): {count} × {} on auction #{auction}", item_text(item)));
        }
        let rows: Vec<(String, i32, i32, i16, i32)> = sqlx::query_as(
            "SELECT c.name::text, c.id, m.market_id, m.count, m.item_id FROM market_result_items m JOIN characters c ON c.id = m.character_id
             WHERE m.item_id = ANY($1) ORDER BY m.market_id",
        )
        .bind(items)
        .fetch_all(&mut *conn)
        .await?;
        for (name, id, market, count, item) in rows {
            out.push(format!("character {name} ({id}): {count} × {} waiting from auction #{market}", item_text(item)));
        }
        let rows: Vec<(String, i32, i16, i16, i16, i32)> = sqlx::query_as(
            "SELECT a.username::text, p.user_id, p.server_id, p.slot, p.count, p.item_id FROM point_items p JOIN accounts a ON a.id = p.user_id
             WHERE p.item_id = ANY($1) ORDER BY a.username, p.slot",
        )
        .bind(items)
        .fetch_all(&mut *conn)
        .await?;
        for (name, id, server, slot, count, item) in rows {
            out.push(format!("account {name} ({id}): {count} × {} in the mall storage of server {server}, slot {slot}", item_text(item)));
        }
        let rows: Vec<(String, i32, i16, i16, i32)> = sqlx::query_as(
            "SELECT a.username::text, b.user_id, b.slot, b.count, b.item_id FROM billing_items b JOIN accounts a ON a.id = b.user_id
             WHERE b.item_id = ANY($1) ORDER BY a.username, b.slot",
        )
        .bind(items)
        .fetch_all(&mut *conn)
        .await?;
        for (name, id, slot, count, item) in rows {
            out.push(format!("account {name} ({id}): {count} × {} in the billing storage, slot {slot}", item_text(item)));
        }
    }
    if !skills.is_empty() {
        let (ids, levels): (Vec<i32>, Vec<i32>) = skills.iter().copied().unzip();
        let rows: Vec<(String, i32, i16, i16)> = sqlx::query_as(
            "SELECT c.name::text, c.id, k.skill_id, k.level FROM character_skills k JOIN characters c ON c.id = k.character_id
             WHERE (k.skill_id::int, k.level::int) IN (SELECT * FROM unnest($1::int[], $2::int[])) ORDER BY c.name, k.skill_id, k.level",
        )
        .bind(&ids)
        .bind(&levels)
        .fetch_all(&mut *conn)
        .await?;
        for (name, id, skill, level) in rows {
            out.push(format!("character {name} ({id}): knows skill {skill}/{level}"));
        }
        let rows: Vec<(String, i32, i16, i16, i32)> = sqlx::query_as(
            "SELECT c.name::text, c.id, k.skill_id, k.level, k.seconds FROM character_applied_skills k JOIN characters c ON c.id = k.character_id
             WHERE (k.skill_id::int, k.level::int) IN (SELECT * FROM unnest($1::int[], $2::int[])) ORDER BY c.name, k.skill_id, k.level",
        )
        .bind(&ids)
        .bind(&levels)
        .fetch_all(&mut *conn)
        .await?;
        for (name, id, skill, level, seconds) in rows {
            out.push(format!("character {name} ({id}): has the buff {skill}/{level} on, {seconds} s left"));
        }
    }
    Ok(out)
}

/// Whether a game server reports players with a fresh heartbeat (an online player's memory would write a deleted item back).
pub async fn players_online(conn: &mut PgConnection) -> Result<i64> {
    Ok(sqlx::query_scalar(sql(format!(
        "SELECT coalesce(sum(players), 0)::bigint FROM game_servers WHERE players > 0 AND last_seen > now() - interval '{HEARTBEAT_WINDOW_SECS} seconds'"
    )))
    .fetch_one(&mut *conn)
    .await?)
}

/// `--force`: deletes the held rows (and the quick slots that point at a deleted bag slot), returning one JSON value per deleted row for
/// the log. Quick slots of skills are left to the client, as everywhere else (`docs/original/item-use.md`).
pub async fn force_delete(conn: &mut PgConnection, items: &[i32], skills: &[(i32, i32)]) -> Result<Vec<Value>> {
    let mut log = Vec::new();
    if !items.is_empty() {
        sqlx::query(
            "DELETE FROM character_quick_slots q USING character_items i
             WHERE i.item_id = ANY($1) AND q.character_id = i.character_id AND q.bag = i.bag AND q.number = i.slot",
        )
        .bind(items)
        .execute(&mut *conn)
        .await?;
        for (table, key) in [
            ("character_items", "character_id, bag, slot"),
            ("bank_items", "user_id, server_id, slot"),
            ("guild_bank_items", "guild_id, slot"),
            ("market_auctions", "id"),
            ("market_result_items", "market_id"),
            ("point_items", "user_id, server_id, slot"),
            ("billing_items", "user_id, slot"),
        ] {
            let rows: Vec<Value> = sqlx::query_scalar(sql(format!(
                "WITH gone AS (DELETE FROM {table} WHERE item_id = ANY($1) RETURNING {key}, item_id) SELECT to_jsonb(gone) FROM gone"
            )))
            .bind(items)
            .fetch_all(&mut *conn)
            .await?;
            log.extend(rows.into_iter().map(|row| json!({"table": table, "row": row})));
        }
    }
    if !skills.is_empty() {
        let (ids, levels): (Vec<i32>, Vec<i32>) = skills.iter().copied().unzip();
        for (table, key) in [("character_skills", "character_id, number"), ("character_applied_skills", "character_id, position")] {
            let rows: Vec<Value> = sqlx::query_scalar(sql(format!(
                "WITH gone AS (DELETE FROM {table} WHERE (skill_id::int, level::int) IN (SELECT * FROM unnest($1::int[], $2::int[])) RETURNING {key}, skill_id, level)
                 SELECT to_jsonb(gone) FROM gone"
            )))
            .bind(&ids)
            .bind(&levels)
            .fetch_all(&mut *conn)
            .await?;
            log.extend(rows.into_iter().map(|row| json!({"table": table, "row": row})));
        }
    }
    Ok(log)
}

/// The columns the reference check reads: `(schema kind, table, columns)`; `pairs` are (type, type id) columns that spell an item id
/// together, `triples` are arrays of (type, type id, count) × 3. A test lists them, so a table this misses is added on purpose.
pub struct RefColumns {
    pub side: &'static str,
    pub table: &'static str,
    /// An SQL expression for the row's key, as the report shows it.
    pub key: &'static str,
    pub kind: RefKind,
}

pub enum RefKind {
    /// Columns holding an item id.
    Ids(&'static [&'static str]),
    /// A (type, type id) pair of columns.
    Pair(&'static str, &'static str),
    /// A smallint array of 3 × (type, type id, count).
    Triples(&'static str),
}

pub const ITEM_REFERENCES: &[RefColumns] = &[
    RefColumns { side: "defs", table: "base_items_defs", key: "rowid", kind: RefKind::Ids(&["itemid"]) },
    RefColumns { side: "defs", table: "mobs", key: "mobid", kind: RefKind::Ids(&["questitemid"]) },
    RefColumns { side: "world", table: "npc_shop_items", key: "npc_type_id || '/' || seq", kind: RefKind::Pair("item_type", "item_type_id") },
    RefColumns { side: "world", table: "quests", key: "id", kind: RefKind::Pair("start_item_type", "start_item_id") },
    RefColumns { side: "world", table: "quests", key: "id", kind: RefKind::Triples("start_items") },
    RefColumns { side: "world", table: "quests", key: "id", kind: RefKind::Triples("end_items") },
    RefColumns { side: "world", table: "quest_results", key: "quest_id || '/' || seq", kind: RefKind::Pair("item_type", "item_type_id") },
    RefColumns { side: "world", table: "item_quest_links", key: "item_type || '/' || item_type_id", kind: RefKind::Pair("item_type", "item_type_id") },
];

/// The mall products' item columns (`itemid1` … `itemidN`), read from the live table.
pub async fn product_columns(conn: &mut PgConnection, s: &Schemas) -> Result<Vec<String>> {
    let mut columns: Vec<(String, i32)> = sqlx::query_as(
        "SELECT column_name::text, regexp_replace(column_name, '^itemid', '')::int FROM information_schema.columns
         WHERE table_schema = $1 AND table_name = 'product_list' AND column_name ~ '^itemid[0-9]+$'",
    )
    .bind(&s.defs)
    .fetch_all(&mut *conn)
    .await?;
    columns.sort_by_key(|c| c.1);
    Ok(columns.into_iter().map(|c| c.0).collect())
}

/// The rows that name the items (`items` are item ids) or give the skills (`skills`: id, level; `own_items`: the extension's own items,
/// which go with it), as `table key column = value` lines, and the edits in force on the owned rows: `owned` is `(tbl, key)`.
pub async fn references(
    conn: &mut PgConnection,
    s: &Schemas,
    items: &[i32],
    skills: &[(i32, i32)],
    owned: &[(String, String)],
) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if !items.is_empty() {
        let schema = |side: &str| if side == "defs" { s.defs.clone() } else { s.world.clone() };
        for column in product_columns(conn, s).await? {
            let rows: Vec<(String, i32)> =
                sqlx::query_as(sql(format!("SELECT productcode::text, {column} FROM {}.product_list WHERE {column} = ANY($1) ORDER BY rowid", s.defs)))
                    .bind(items)
                    .fetch_all(&mut *conn)
                    .await?;
            for (code, id) in rows {
                out.push(format!("defs.product_list {code} {column} = {id}"));
            }
        }
        for r in ITEM_REFERENCES {
            let table = format!("{}.{}", schema(r.side), r.table);
            let key = r.key;
            match &r.kind {
                RefKind::Ids(columns) => {
                    for column in *columns {
                        let rows: Vec<(String, i32)> = sqlx::query_as(sql(format!("SELECT ({key})::text, {column}::int FROM {table} WHERE {column} = ANY($1) ORDER BY 1")))
                            .bind(items)
                            .fetch_all(&mut *conn)
                            .await?;
                        out.extend(rows.into_iter().map(|(k, id)| format!("{}.{} {k} {column} = {id}", r.side, r.table)));
                    }
                }
                RefKind::Pair(t, i) => {
                    let rows: Vec<(String, i32)> = sqlx::query_as(sql(format!(
                        "SELECT ({key})::text, ({t}::int * 1000 + {i}::int) FROM {table} WHERE ({t}::int * 1000 + {i}::int) = ANY($1) ORDER BY 1"
                    )))
                    .bind(items)
                    .fetch_all(&mut *conn)
                    .await?;
                    out.extend(rows.into_iter().map(|(k, id)| format!("{}.{} {k} {t}/{i} = {id}", r.side, r.table)));
                }
                RefKind::Triples(column) => {
                    let rows: Vec<(String, i32)> = sqlx::query_as(sql(format!(
                        "SELECT ({key})::text, ({column}[3 * k + 1]::int * 1000 + {column}[3 * k + 2]::int) FROM {table}, generate_series(0, 2) k
                         WHERE ({column}[3 * k + 1]::int * 1000 + {column}[3 * k + 2]::int) = ANY($1) ORDER BY 1, 2"
                    )))
                    .bind(items)
                    .fetch_all(&mut *conn)
                    .await?;
                    out.extend(rows.into_iter().map(|(k, id)| format!("{}.{} {k} {column} = {id}", r.side, r.table)));
                }
            }
        }
    }
    if !skills.is_empty() {
        let (ids, levels): (Vec<i32>, Vec<i32>) = skills.iter().copied().unzip();
        // An item whose range and attacktime name a skill gives it when used (item_use.rs, Effect::Skill; special 50 is an exp wheel).
        let own: Vec<i32> = owned.iter().filter(|(t, _)| t == "defs.items").filter_map(|(_, k)| k.parse().ok()).collect();
        let rows: Vec<(i32, i32, i32)> = sqlx::query_as(sql(format!(
            "SELECT itemid, range::int, attacktime::int FROM {}.items
             WHERE range <> 0 AND attacktime <> 0 AND special <> 50 AND NOT (itemid = ANY($3))
               AND (range::int, attacktime::int) IN (SELECT * FROM unnest($1::int[], $2::int[])) ORDER BY itemid",
            s.defs
        )))
        .bind(&ids)
        .bind(&levels)
        .bind(&own)
        .fetch_all(&mut *conn)
        .await?;
        for (item, skill, level) in rows {
            out.push(format!("defs.items {item} gives the skill {skill}/{level} (range, attacktime)"));
        }
    }
    // Edits in force on rows it owns: `item_grade` on defs.items, `mob_drop` on defs.mob_items.
    for (tbl, key) in owned {
        let Some(table) = tbl.strip_prefix("defs.") else { continue };
        for target in crate::registry::all().iter().filter(|t| t.table == table) {
            let fields: Vec<(String, i32)> = sqlx::query_as(sql(format!(
                "SELECT field, change_set FROM {}.fields WHERE target = $1 AND key = $2 ORDER BY field",
                s.edits
            )))
            .bind(target.name)
            .bind(key)
            .fetch_all(&mut *conn)
            .await?;
            for (field, cs) in fields {
                out.push(format!("edit {} {key} {field} (change set #{cs})", target.name));
            }
            let rows: Vec<(String, i32)> =
                sqlx::query_as(sql(format!("SELECT action, change_set FROM {}.rows WHERE target = $1 AND key = $2", s.edits)))
                    .bind(target.name)
                    .bind(key)
                    .fetch_all(&mut *conn)
                    .await?;
            for (action, cs) in rows {
                out.push(format!("edit {} {key} row {action} (change set #{cs})", target.name));
            }
        }
    }
    Ok(out)
}
