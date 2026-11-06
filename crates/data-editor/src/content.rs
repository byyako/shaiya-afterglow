//! `data-editor content …` (`docs/plans/ext-1-foundation.md` sections 3.2 and 3.5): the live `defs` and `world` are built from the frozen
//! originals, the installed extensions' rows and the data editor's edits, in that order; `check` proves the live tables are exactly that,
//! `status` says what is imported and installed, `hashes` is the proof of the build, `origin` shows the layers of one row.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::{AssertSqlSafe, PgConnection};

use crate::apply::{self, Report};
use crate::ext::tables::{self, ExtTable};
use crate::registry::{self, Schemas, Side};
use crate::store::{Editor, refuse};

const DEFS_DDL: &str = include_str!("../../../db/defs/schema.sql");
const WORLD_DDL: &str = include_str!("../../../db/world/schema.sql");

fn sql(text: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(text)
}

/// `defs` or `world`, as `content.imports.part` and the commands spell it.
pub fn part_name(side: Side) -> &'static str {
    match side {
        Side::Defs => "defs",
        Side::World => "world",
        Side::Content => "content",
    }
}

/// What one part's rebuild did.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PartResult {
    pub part: String,
    /// The live schema was missing and has been created from the DDL.
    pub created: bool,
    pub tables: Vec<(String, i64)>,
    /// Rows written for installed extensions.
    pub ext_rows: usize,
    pub report: Report,
}

// ------------------------------------------------------------------------------------------------------------------ small queries

/// The ordinary tables of a schema, without the edits marker.
pub async fn tables_of(conn: &mut PgConnection, schema: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = $1 AND c.relkind IN ('r', 'p') AND c.relname <> 'edits_applied' ORDER BY c.relname",
    )
    .bind(schema)
    .fetch_all(&mut *conn)
    .await?)
}

pub async fn schema_exists(conn: &mut PgConnection, schema: &str) -> Result<bool> {
    Ok(sqlx::query_scalar("SELECT to_regnamespace($1::text) IS NOT NULL").bind(schema).fetch_one(&mut *conn).await?)
}

pub async fn columns_of(conn: &mut PgConnection, schema: &str, table: &str) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns WHERE table_schema = $1 AND table_name = $2 ORDER BY ordinal_position",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(&mut *conn)
    .await?)
}

/// `(rows, md5)` of a table: the md5 over the rows' text in a stable order, the same as `tools/content/table_hashes.sql`.
pub async fn table_hash(conn: &mut PgConnection, schema: &str, table: &str) -> Result<(i64, String)> {
    Ok(sqlx::query_as(sql(format!(
        "SELECT count(*)::bigint, coalesce(md5(string_agg(t::text, chr(10) ORDER BY t::text)), md5('')) FROM {schema}.{table} t"
    )))
    .fetch_one(&mut *conn)
    .await?)
}

/// Foreign-key parents before children (a table with a key to itself counts as having no parent).
async fn insert_order(conn: &mut PgConnection, schema: &str, tables: &[String]) -> Result<Vec<String>> {
    let pairs: Vec<(String, String)> = sqlx::query_as(
        "SELECT c1.relname::text, c2.relname::text FROM pg_constraint k JOIN pg_class c1 ON c1.oid = k.conrelid
         JOIN pg_class c2 ON c2.oid = k.confrelid JOIN pg_namespace n ON n.oid = c1.relnamespace
         WHERE k.contype = 'f' AND n.nspname = $1 AND c1.oid <> c2.oid",
    )
    .bind(schema)
    .fetch_all(&mut *conn)
    .await?;
    let mut done: Vec<String> = Vec::new();
    let mut left: Vec<String> = tables.to_vec();
    while !left.is_empty() {
        let (ready, rest): (Vec<String>, Vec<String>) =
            left.into_iter().partition(|t| pairs.iter().filter(|(child, _)| child == t).all(|(_, parent)| done.contains(parent)));
        if ready.is_empty() {
            bail!("the foreign keys of {schema} form a cycle: {rest:?}");
        }
        done.extend(ready);
        left = rest;
    }
    Ok(done)
}

// ------------------------------------------------------------------------------------------------------------------------ rebuild

fn ddl_for(side: Side, schema: &str) -> String {
    match side {
        Side::Defs => DEFS_DDL
            .replace("CREATE SCHEMA IF NOT EXISTS defs;", &format!("CREATE SCHEMA {schema};"))
            .replace("defs.", &format!("{schema}.")),
        Side::World => {
            WORLD_DDL.replace("CREATE SCHEMA world;", &format!("CREATE SCHEMA {schema};")).replace("world.", &format!("{schema}."))
        }
        Side::Content => unreachable!("content is never rebuilt from a frozen copy"),
    }
}

/// Writes the rows of installed extensions' `content.ext_rows` entry into the live table (`docs/plans/ext-1-foundation.md` 5.7 step 4).
/// A change updates the row with the key; an add inserts it. The jsonb is the whole row as it was computed at install.
pub async fn write_ext_row(conn: &mut PgConnection, s: &Schemas, t: &ExtTable, action: &str, row: &Value) -> Result<()> {
    let schema = s.of(t.side);
    let live = format!("{schema}.{}", t.table);
    let cols = columns_of(conn, schema, t.table).await?;
    let list = cols.join(", ");
    match action {
        "add" => {
            sqlx::query(sql(format!("INSERT INTO {live} ({list}) SELECT {list} FROM jsonb_populate_record(NULL::{live}, $1::jsonb)")))
                .bind(row)
                .execute(&mut *conn)
                .await?;
        }
        "change" => {
            let set = cols.iter().map(|c| format!("{c} = r.{c}")).collect::<Vec<_>>().join(", ");
            let on = t.key_cols.iter().map(|c| format!("t.{c} = r.{c}")).collect::<Vec<_>>().join(" AND ");
            let done =
                sqlx::query(sql(format!("UPDATE {live} t SET {set} FROM jsonb_populate_record(NULL::{live}, $1::jsonb) r WHERE {on}")))
                    .bind(row)
                    .execute(&mut *conn)
                    .await?
                    .rows_affected();
            if done != 1 {
                bail!("{} {}: the row an extension changes is not there ({done} rows updated)", t.tbl, row_key(t, row));
            }
        }
        other => bail!("ext_rows action {other:?}"),
    }
    Ok(())
}

fn row_key(t: &ExtTable, row: &Value) -> String {
    t.key_cols.iter().map(|c| row.get(*c).map(|v| v.to_string()).unwrap_or_default()).collect::<Vec<_>>().join("/")
}

/// The rebuild of one part inside the caller's transaction: steps 1 to 7 of section 3.2. A missing live schema is made from the DDL, or,
/// for a scratch rebuild (`like`: the schema it stands in for), as a structural copy of that schema.
pub async fn rebuild_part(conn: &mut PgConnection, s: &Schemas, side: Side, like: Option<&str>) -> Result<PartResult> {
    let part = part_name(side);
    let imported: bool = sqlx::query_scalar(sql(format!("SELECT EXISTS (SELECT 1 FROM {}.imports WHERE part = $1)", s.content)))
        .bind(part)
        .fetch_one(&mut *conn)
        .await?;
    if !imported {
        return refuse(format!(
            "{part} has no import record: run tools/setup.sh with the original files first (a database from before 0.3.0: docs/plans/ext-1-foundation.md decision 11)"
        ));
    }
    let frozen = s.original_of(side).to_owned();
    let live = s.of(side).to_owned();
    if !schema_exists(conn, &frozen).await? {
        bail!("content.imports says {part} was imported, but the schema {frozen} does not exist");
    }
    let mut result = PartResult { part: part.to_owned(), ..Default::default() };
    // 2. the live schema: made from the DDL when missing, else emptied in one TRUNCATE.
    if !schema_exists(conn, &live).await? {
        match like {
            Some(model) if schema_exists(conn, model).await? => {
                sqlx::query(sql(format!("CREATE SCHEMA {live}"))).execute(&mut *conn).await?;
                for table in tables_of(conn, model).await? {
                    sqlx::query(sql(format!("CREATE TABLE {live}.{table} (LIKE {model}.{table} INCLUDING ALL)")))
                        .execute(&mut *conn)
                        .await?;
                }
            }
            _ => {
                sqlx::raw_sql(sql(ddl_for(side, &live)))
                    .execute(&mut *conn)
                    .await
                    .with_context(|| format!("creating {live} from its DDL"))?;
            }
        }
        result.created = true;
    } else {
        let all = tables_of(conn, &live).await?;
        if !all.is_empty() {
            let list = all.iter().map(|t| format!("{live}.{t}")).collect::<Vec<_>>().join(", ");
            sqlx::query(sql(format!("TRUNCATE {list}"))).execute(&mut *conn).await?;
        }
    }
    // 3. the originals, in foreign-key order; identity sequences past the highest id.
    let frozen_tables = tables_of(conn, &frozen).await?;
    let live_tables = tables_of(conn, &live).await?;
    if let Some(t) = frozen_tables.iter().find(|t| !live_tables.contains(t)) {
        bail!("{frozen}.{t} has no counterpart in {live}: the frozen copy and the live schema come from different DDL");
    }
    for table in insert_order(conn, &live, &frozen_tables).await? {
        let cols = columns_of(conn, &frozen, &table).await?;
        let list = cols.join(", ");
        let rows = sqlx::query(sql(format!("INSERT INTO {live}.{table} ({list}) SELECT {list} FROM {frozen}.{table}")))
            .execute(&mut *conn)
            .await?;
        result.tables.push((table.clone(), rows.rows_affected() as i64));
        let identities: Vec<String> = sqlx::query_scalar(
            "SELECT a.attname::text FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = $1 AND c.relname = $2 AND a.attnum > 0 AND NOT a.attisdropped AND a.attidentity <> ''",
        )
        .bind(&live)
        .bind(&table)
        .fetch_all(&mut *conn)
        .await?;
        for col in identities {
            sqlx::query(sql(format!(
                "SELECT setval(pg_get_serial_sequence('{live}.{table}', '{col}'), (SELECT coalesce(max({col}), 1) FROM {live}.{table}))"
            )))
            .execute(&mut *conn)
            .await?;
        }
    }
    // 4. the edits' marker goes: the part is fresh.
    sqlx::query(sql(format!("DROP TABLE IF EXISTS {live}.edits_applied"))).execute(&mut *conn).await?;
    // 5. the rows of every installed extension (enabled or not), in install order.
    let ext_rows: Vec<(String, String, String, Value)> = sqlx::query_as(sql(format!(
        "SELECT r.tbl, r.key, r.action, r.row FROM {c}.ext_rows r JOIN {c}.extensions e ON e.name = r.extension
         WHERE r.tbl LIKE $1 ORDER BY e.install_seq, r.seq",
        c = s.content
    )))
    .bind(format!("{part}.%"))
    .fetch_all(&mut *conn)
    .await?;
    for (tbl, key, action, row) in &ext_rows {
        let t = tables::find(tbl).with_context(|| format!("content.ext_rows holds a row of {tbl}, which no extension may touch"))?;
        write_ext_row(conn, s, t, action, row).await.with_context(|| format!("writing the extension row {tbl} {key}"))?;
    }
    result.ext_rows = ext_rows.len();
    // 6. the edits.
    apply::apply_side(conn, s, side, &mut result.report).await?;
    // 7. done.
    sqlx::query(sql(format!("UPDATE {}.imports SET rebuilt_at = now() WHERE part = $1", s.content))).bind(part).execute(&mut *conn).await?;
    Ok(result)
}

/// The schema names of a scratch rebuild: `<prefix>_defs` and `<prefix>_world` take the place of the live schemas.
pub fn scratch(s: &Schemas, prefix: &str) -> Schemas {
    Schemas { defs: format!("{prefix}_defs"), world: format!("{prefix}_world"), ..s.clone() }
}

// --------------------------------------------------------------------------------------------------------------------------- check

fn short(v: &Value) -> String {
    let text = match v {
        Value::String(s) => format!("'{s}'"),
        other => other.to_string(),
    };
    if text.chars().count() > 60 { format!("{}…", text.chars().take(60).collect::<String>()) } else { text }
}

/// The key columns used to pair rows in a report: the primary key, else the extendable table's, else the editor target's.
async fn key_columns(conn: &mut PgConnection, schema: &str, table: &str, side: Side) -> Result<Vec<String>> {
    let pk: Vec<String> = sqlx::query_scalar(
        "SELECT a.attname::text FROM pg_index i JOIN pg_class c ON c.oid = i.indrelid JOIN pg_namespace n ON n.oid = c.relnamespace
         JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = ANY (i.indkey)
         WHERE i.indisprimary AND n.nspname = $1 AND c.relname = $2 ORDER BY array_position(i.indkey::int2[], a.attnum)",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(&mut *conn)
    .await?;
    if !pk.is_empty() {
        return Ok(pk);
    }
    if let Some(t) = tables::TABLES.iter().find(|t| t.side == side && t.table == table) {
        return Ok(t.key_cols.iter().map(|c| (*c).to_owned()).collect());
    }
    Ok(registry::all()
        .iter()
        .find(|t| t.side == side && t.table == table)
        .map(|t| t.key_cols.iter().map(|c| (*c).to_owned()).collect())
        .unwrap_or_default())
}

/// What differs between the live table and the table a rebuild gives (`EXCEPT ALL` both ways), worded as section 3.5 words it.
async fn diff_table(
    conn: &mut PgConnection,
    label: &str,
    live: &str,
    expected: &str,
    table: &str,
    side: Side,
    out: &mut Vec<String>,
) -> Result<()> {
    let only_live: Vec<Value> = sqlx::query_scalar(sql(format!(
        "SELECT to_jsonb(l) FROM {live}.{table} l EXCEPT ALL SELECT to_jsonb(e) FROM {expected}.{table} e LIMIT 50"
    )))
    .fetch_all(&mut *conn)
    .await?;
    let only_expected: Vec<Value> = sqlx::query_scalar(sql(format!(
        "SELECT to_jsonb(e) FROM {expected}.{table} e EXCEPT ALL SELECT to_jsonb(l) FROM {live}.{table} l LIMIT 50"
    )))
    .fetch_all(&mut *conn)
    .await?;
    if only_live.is_empty() && only_expected.is_empty() {
        return Ok(());
    }
    let keys = key_columns(conn, live, table, side).await?;
    let key_of = |row: &Value| {
        keys.iter()
            .map(|k| row.get(k).map(|v| v.to_string().trim_matches('"').to_owned()).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("/")
    };
    let mut matched = vec![false; only_expected.len()];
    for row in &only_live {
        let key = if keys.is_empty() { short(row) } else { key_of(row) };
        let pair = if keys.is_empty() { None } else { only_expected.iter().position(|e| key_of(e) == key) };
        match pair {
            Some(i) => {
                matched[i] = true;
                let (a, b) = (row.as_object().cloned().unwrap_or_default(), only_expected[i].as_object().cloned().unwrap_or_default());
                for (col, value) in &a {
                    if b.get(col) != Some(value) {
                        out.push(format!(
                            "{label}.{table} {key}: hand edit? {col} live {}, expected {}",
                            short(value),
                            short(b.get(col).unwrap_or(&Value::Null))
                        ));
                    }
                }
            }
            None => out.push(format!("{label}.{table} {key}: hand edit? a row that a rebuild does not give is in the live table")),
        }
    }
    for (i, row) in only_expected.iter().enumerate() {
        if !matched[i] {
            let key = if keys.is_empty() { short(row) } else { key_of(row) };
            out.push(format!("{label}.{table} {key}: hand edit? a row that a rebuild gives is not in the live table"));
        }
    }
    Ok(())
}

impl Editor {
    /// `content rebuild [defs|world|all]`: one transaction, under the same lock as Apply.
    pub async fn content_rebuild(&self, parts: &[Side]) -> Result<Vec<PartResult>> {
        let mut tx = self.begin_locked().await?;
        let mut out = Vec::new();
        for side in parts {
            out.push(rebuild_part(&mut tx, &self.s, *side, None).await?);
        }
        tx.commit().await?;
        Ok(out)
    }

    /// `content rebuild --into <prefix>`: the same into scratch schemas, rolled back (what the rebuild would write and report).
    pub async fn content_rebuild_into(&self, parts: &[Side], prefix: &str) -> Result<Vec<PartResult>> {
        let mut tx = self.begin_locked().await?;
        let scratch = scratch(&self.s, prefix);
        let mut out = Vec::new();
        for side in parts {
            out.push(rebuild_part(&mut tx, &scratch, *side, Some(self.s.of(*side))).await?);
        }
        tx.rollback().await?;
        Ok(out)
    }

    /// `content hashes`: `schema.table|rows|md5` per ordinary table of the schemas that exist (default `defs world client`).
    pub async fn content_hashes(&self, schemas: &[String]) -> Result<Vec<String>> {
        let defaults;
        let schemas = if schemas.is_empty() {
            defaults = [self.s.defs.clone(), self.s.world.clone(), self.s.client.clone()];
            &defaults[..]
        } else {
            schemas
        };
        let mut conn = self.pool.acquire().await?;
        let mut lines = Vec::new();
        for schema in schemas {
            if !schema_exists(&mut conn, schema).await? {
                continue;
            }
            for table in tables_of(&mut conn, schema).await? {
                let (rows, md5) = table_hash(&mut conn, schema, &table).await?;
                lines.push(format!("{schema}.{table}|{rows}|{md5}"));
            }
        }
        Ok(lines)
    }

    /// `content check`: every finding, one line each (an empty list: clean).
    pub async fn content_check(&self) -> Result<Vec<String>> {
        let s = &self.s;
        let c = &s.content;
        let mut tx = self.begin_locked().await?;
        let mut findings = Vec::new();
        let imports: Vec<(String, Value)> =
            sqlx::query_as(sql(format!("SELECT part, tables FROM {c}.imports ORDER BY part"))).fetch_all(&mut *tx).await?;
        // 1. the frozen copies are as imported.
        for (part, recorded) in &imports {
            let (schema, only): (&str, Option<&[&str]>) = match part.as_str() {
                "defs" => (&s.original_defs, None),
                "world" => (&s.original_world, None),
                _ => (&s.client, Some(&["items", "skills"])),
            };
            if !schema_exists(&mut tx, schema).await? {
                findings.push(format!("{part}: the frozen copy {schema} does not exist"));
                continue;
            }
            let have = tables_of(&mut tx, schema).await?;
            let recorded = recorded.as_object().cloned().unwrap_or_default();
            for (table, want) in &recorded {
                if !have.contains(table) {
                    findings.push(format!("{schema}.{table}: the frozen table is gone"));
                    continue;
                }
                let (rows, md5) = table_hash(&mut tx, schema, table).await?;
                let (want_rows, want_md5) = (want["rows"].as_i64().unwrap_or(-1), want["md5"].as_str().unwrap_or(""));
                if rows != want_rows || md5 != want_md5 {
                    findings.push(format!("{schema}.{table}: the frozen copy changed since the import ({rows} rows, md5 {md5}; recorded {want_rows} rows, md5 {want_md5})"));
                }
            }
            if only.is_none() {
                for table in have.iter().filter(|t| !recorded.contains_key(*t)) {
                    findings.push(format!("{schema}.{table}: a table the import did not record"));
                }
            }
        }
        // 2. a rebuild into scratch schemas equals the live tables.
        let scratch = scratch(s, &format!("check_{:08x}", rand::random::<u32>()));
        for side in [Side::Defs, Side::World] {
            let part = part_name(side);
            if !imports.iter().any(|(p, _)| p == part) {
                continue;
            }
            let live = s.of(side).to_owned();
            if !schema_exists(&mut tx, &live).await? {
                findings.push(format!("{part}: the live schema {live} does not exist (data-editor content rebuild {part})"));
                continue;
            }
            rebuild_part(&mut tx, &scratch, side, Some(&live)).await?;
            let expected = scratch.of(side).to_owned();
            let live_tables = tables_of(&mut tx, &live).await?;
            let expected_tables = tables_of(&mut tx, &expected).await?;
            for table in &expected_tables {
                if !live_tables.contains(table) {
                    findings.push(format!("{part}.{table}: the live table is missing"));
                    continue;
                }
                diff_table(&mut tx, part, &live, &expected, table, side, &mut findings).await?;
            }
            for table in live_tables.iter().filter(|t| !expected_tables.contains(t)) {
                findings.push(format!("{part}.{table}: a live table the originals do not have"));
            }
        }
        // 3. every owned key lies in its extension's ranges.
        let owned: Vec<(String, String, String)> =
            sqlx::query_as(sql(format!("SELECT extension, tbl, key FROM {c}.ext_rows ORDER BY extension, tbl, key")))
                .fetch_all(&mut *tx)
                .await?;
        for (extension, tbl, key) in owned {
            let Ok(parts) = tables::parse_key(&key) else {
                findings.push(format!("{tbl} {key}: owned by {extension}, but not a key"));
                continue;
            };
            let (kind, major, minor) = match (tbl.as_str(), parts.as_slice()) {
                ("defs.items", [id]) => ("item", id / 1000, id % 1000),
                ("defs.skills", [id, level]) => ("skill", *id, *level),
                _ => continue, // drop entries are owned per row: no range
            };
            let inside: bool = sqlx::query_scalar(sql(format!(
                "SELECT EXISTS (SELECT 1 FROM {c}.ext_ranges WHERE extension = $1 AND kind = $2 AND major = $3 AND $4 BETWEEN minor_low AND minor_high)"
            )))
            .bind(&extension)
            .bind(kind)
            .bind(major as i32)
            .bind(minor as i32)
            .fetch_one(&mut *tx)
            .await?;
            if !inside {
                findings.push(format!("{tbl} {key}: owned by {extension}, outside the ids it declares"));
            }
        }
        // 4. no two enabled extensions set the same setting.
        let clashes: Vec<(String, Vec<String>)> = sqlx::query_as(sql(format!(
            "SELECT s.name, array_agg(s.extension ORDER BY s.extension) FROM {c}.ext_settings s JOIN {c}.extensions e ON e.name = s.extension
             WHERE e.enabled GROUP BY s.name HAVING count(*) > 1"
        )))
        .fetch_all(&mut *tx)
        .await?;
        for (name, who) in clashes {
            findings.push(format!("setting {name}: set by the enabled extensions {}", who.join(", ")));
        }
        tx.rollback().await?;
        Ok(findings)
    }

    /// `content status`, as lines of text.
    pub async fn content_status(&self) -> Result<Vec<String>> {
        let c = &self.s.content;
        let mut conn = self.pool.acquire().await?;
        let mut out = Vec::new();
        let imports: Vec<(String, String, String, Value, Option<String>, bool)> = sqlx::query_as(sql(format!(
            "SELECT part, imported_at::text, source, tables, rebuilt_at::text, (rebuilt_at IS NULL OR rebuilt_at < imported_at) FROM {c}.imports ORDER BY part"
        )))
        .fetch_all(&mut *conn)
        .await?;
        if imports.is_empty() {
            out.push("no original data is recorded as imported: run tools/setup.sh with the original files (docs/plans/ext-1-foundation.md decision 11)".to_owned());
        }
        for (part, at, source, tables, rebuilt, due) in &imports {
            let map = tables.as_object().cloned().unwrap_or_default();
            let rows: i64 = map.values().filter_map(|t| t["rows"].as_i64()).sum();
            let rebuilt = match (part.as_str(), rebuilt, due) {
                ("client", _, _) => String::new(),
                (_, None, _) => "; rebuild DUE (never run)".to_owned(),
                (_, Some(at), true) => format!("; rebuild DUE (last {at}, before the import)"),
                (_, Some(at), false) => format!("; rebuilt {at}"),
            };
            out.push(format!("{part}: imported {at} from {source}; {} tables, {rows} rows{rebuilt}", map.len()));
        }
        let extensions: Vec<(String, String, bool, i64, i64, i64)> = sqlx::query_as(sql(format!(
            "SELECT e.name, e.version, e.enabled,
                    (SELECT count(*) FROM {c}.ext_rows r WHERE r.extension = e.name),
                    (SELECT count(*) FROM {c}.client_items i WHERE i.extension = e.name) + (SELECT count(*) FROM {c}.client_skills k WHERE k.extension = e.name),
                    (SELECT count(*) FROM {c}.ext_settings t WHERE t.extension = e.name)
             FROM {c}.extensions e ORDER BY e.install_seq"
        )))
        .fetch_all(&mut *conn)
        .await?;
        if extensions.is_empty() {
            out.push("extensions: none installed".to_owned());
        }
        for (name, version, enabled, rows, client_rows, settings) in extensions {
            out.push(format!(
                "extension {name} {version}: {}, {rows} rows, {client_rows} client rows, {settings} settings",
                if enabled { "enabled" } else { "disabled" }
            ));
        }
        let unshipped: i64 = sqlx::query_scalar(sql(format!(
            "SELECT (SELECT count(*) FROM {c}.client_items WHERE since_patch IS NULL) + (SELECT count(*) FROM {c}.client_skills WHERE since_patch IS NULL)"
        )))
        .fetch_one(&mut *conn)
        .await?;
        let withdrawn: i64 = sqlx::query_scalar(sql(format!(
            "SELECT count(*) FROM {c}.ext_log l WHERE l.action = 'remove' AND coalesce((l.detail->>'shipped_client_rows')::int, 0) > 0
               AND l.at > coalesce((SELECT max(generated_at) FROM {c}.client_patches), '-infinity')"
        )))
        .fetch_one(&mut *conn)
        .await?;
        if unshipped > 0 || withdrawn > 0 {
            out.push(format!(
                "a client patch is due: {unshipped} client rows are in no patch yet, {withdrawn} removals took out rows an earlier patch carried (tools/sdata/client_patch.py check, then generate)"
            ));
        } else {
            out.push("no client patch is due".to_owned());
        }
        Ok(out)
    }

    /// `content origin <table> <key>`: the layers of one row, as lines of text.
    pub async fn content_origin(&self, table: &str, key: &str) -> Result<Vec<String>> {
        let s = &self.s;
        let c = &s.content;
        let bare = table.rsplit('.').next().unwrap_or(table);
        let (side, key_cols): (Side, Vec<String>) = if let Some(t) = tables::find(bare) {
            (t.side, t.key_cols.iter().map(|c| (*c).to_owned()).collect())
        } else if let Some(t) = registry::all().iter().find(|t| t.table == bare) {
            (t.side, t.key_cols.iter().map(|c| (*c).to_owned()).collect())
        } else {
            return refuse(format!("{table:?} is not a table the editor or the extensions know"));
        };
        let numbers = tables::parse_key(key).map_err(|e| anyhow::anyhow!(e))?;
        if numbers.len() != key_cols.len() {
            return refuse(format!("{bare} has a key of {} parts ({}), not {}", key_cols.len(), key_cols.join("/"), numbers.len()));
        }
        let mut conn = self.pool.acquire().await?;
        let filter = key_cols.iter().zip(&numbers).map(|(c, n)| format!("{c} = {n}")).collect::<Vec<_>>().join(" AND ");
        let row_in = |schema: String| sql(format!("SELECT to_jsonb(t) FROM {schema}.{bare} t WHERE {filter}"));
        let live: Option<Value> = sqlx::query_scalar(row_in(s.of(side).to_owned())).fetch_optional(&mut *conn).await?;
        let original: Option<Value> = if schema_exists(&mut conn, s.original_of(side)).await? {
            sqlx::query_scalar(row_in(s.original_of(side).to_owned())).fetch_optional(&mut *conn).await?
        } else {
            None
        };
        let label = format!("{}.{bare} {key}", part_name(side));
        let mut out = vec![label.clone()];
        let show = |v: &Option<Value>| match v {
            Some(v) => v.to_string(),
            None => "(no such row)".to_owned(),
        };
        out.push(format!("  original ({}): {}", s.original_of(side), show(&original)));
        let owner: Option<(String, String, Value)> =
            sqlx::query_as(sql(format!("SELECT extension, action, row FROM {c}.ext_rows WHERE tbl = $1 AND key = $2")))
                .bind(format!("{}.{bare}", part_name(side)))
                .bind(key)
                .fetch_optional(&mut *conn)
                .await?;
        match &owner {
            Some((extension, action, row)) => {
                let set: Vec<String> = row
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .filter(|(col, v)| original.as_ref().and_then(|o| o.get(col.as_str())) != Some(*v))
                            .map(|(col, v)| format!("{col} = {}", short(v)))
                            .collect()
                    })
                    .unwrap_or_default();
                out.push(format!(
                    "  extension {extension} ({action}): {}",
                    if set.is_empty() { "no column differs from the original".to_owned() } else { set.join(", ") }
                ));
            }
            None => out.push("  extension: none owns this row".to_owned()),
        }
        let mut edits = 0;
        for t in registry::all().iter().filter(|t| t.side == side && t.table == bare) {
            let fields: Vec<(String, Value, Option<Value>, i32)> = sqlx::query_as(sql(format!(
                "SELECT field, value, original, change_set FROM {}.fields WHERE target = $1 AND key = $2 ORDER BY field",
                s.edits
            )))
            .bind(t.name)
            .bind(key)
            .fetch_all(&mut *conn)
            .await?;
            for (field, value, original, change_set) in fields {
                edits += 1;
                out.push(format!(
                    "  edit {} {field} = {value} (original {}; change set #{change_set})",
                    t.name,
                    original.map(|o| o.to_string()).unwrap_or_else(|| "-".to_owned())
                ));
            }
            let rows: Vec<(String, i32)> =
                sqlx::query_as(sql(format!("SELECT action, change_set FROM {}.rows WHERE target = $1 AND key = $2", s.edits)))
                    .bind(t.name)
                    .bind(key)
                    .fetch_all(&mut *conn)
                    .await?;
            for (action, change_set) in rows {
                edits += 1;
                out.push(format!("  edit {} row {action} (change set #{change_set})", t.name));
            }
        }
        if edits == 0 {
            out.push("  edits: none in force".to_owned());
        }
        out.push(format!("  live: {}", show(&live)));
        if let (Some(live), Some(original)) = (&live, &original) {
            let (l, o): (Map<String, Value>, Map<String, Value>) =
                (live.as_object().cloned().unwrap_or_default(), original.as_object().cloned().unwrap_or_default());
            let differing: Vec<String> = l
                .iter()
                .filter(|(col, v)| o.get(*col) != Some(*v))
                .map(|(col, v)| format!("{col}: {} -> {}", short(o.get(col).unwrap_or(&Value::Null)), short(v)))
                .collect();
            out.push(format!(
                "  live against the original: {}",
                if differing.is_empty() { "identical".to_owned() } else { differing.join("; ") }
            ));
        }
        Ok(out)
    }
}
