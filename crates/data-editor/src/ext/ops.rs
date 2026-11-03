//! `data-editor ext …` (`docs/plans/ext-1-foundation.md` section 5.7): install, upgrade, remove, enable, disable, check, list, show.
//! Everything an install reads from the folder goes into the database, so a server runs, and an extension can be removed, without it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, PgConnection};

use super::ids::{self, IdRange};
use super::manifest::Manifest;
use super::refs;
use super::rows::{ClientSpec, Entry, Kind};
use super::tables::ExtTable;
use crate::apply;
use crate::content::{columns_of, schema_exists, table_hash, tables_of, write_ext_row};
use crate::registry::{self, Ctx, Facts, Schemas, Side};
use crate::store::{Editor, refuse};

fn sql(text: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(text)
}

pub fn hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

// ------------------------------------------------------------------------------------------------------------------------- the folder

#[derive(Clone, Debug)]
pub struct Migration {
    pub version: i32,
    pub name: String,
    pub sql: String,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct ArtFile {
    pub member: String,
    pub file: String,
    pub sha256: String,
}

/// Everything an install reads from the extension's folder.
#[derive(Clone, Debug)]
pub struct Plan {
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub entries: Vec<Entry>,
    pub migrations: Vec<Migration>,
    pub art: Vec<ArtFile>,
}

/// Reads and validates what needs no database: the manifest, the rows files, the migrations' names and the art files.
pub fn read_folder(dir: &Path) -> Result<Plan, String> {
    let manifest_path = dir.join("extension.toml");
    let text = std::fs::read_to_string(&manifest_path).map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let manifest = Manifest::parse(&text).map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let entries = super::rows::load_rows(dir)?;
    for e in &entries {
        let ranges = match e.kind {
            Kind::Item => Some((&manifest.items, e.key[0] / 1000, e.key[0] % 1000)),
            Kind::Skill => Some((&manifest.skills, e.key[0], e.key[1])),
            Kind::Drop => None,
        };
        if let Some((list, major, minor)) = ranges
            && !list.iter().any(|r| r.contains(major, minor))
        {
            return Err(format!("{}: {} lies outside the ids [ids] declares for {}s ({})", e.label(), e.id, e.kind.word(), range_texts(list)));
        }
    }
    let mut migrations = Vec::new();
    let mdir = dir.join("migrations");
    if mdir.is_dir() {
        let mut files: Vec<_> = std::fs::read_dir(&mdir)
            .map_err(|e| format!("{}: {e}", mdir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        for (i, path) in files.iter().enumerate() {
            let file = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let stem = file.strip_suffix(".sql").ok_or_else(|| format!("migrations/{file}: a migration is NNNN_<what>.sql"))?;
            let (number, what) = stem.split_once('_').ok_or_else(|| format!("migrations/{file}: a migration is NNNN_<what>.sql"))?;
            let version: i32 = number.parse().ok().filter(|_| number.len() == 4).ok_or_else(|| format!("migrations/{file}: a migration is NNNN_<what>.sql"))?;
            if version != i as i32 + 1 {
                return Err(format!("migrations/{file}: migrations are numbered 0001, 0002, … without gaps (this is number {version}, expected {})", i + 1));
            }
            if what.is_empty() || !what.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
                return Err(format!("migrations/{file}: the name after the number is lower case letters, digits and _"));
            }
            let sql = std::fs::read_to_string(path).map_err(|e| format!("migrations/{file}: {e}"))?;
            migrations.push(Migration { version, name: stem.to_owned(), sha256: hex(sql.as_bytes()), sql });
        }
    }
    let mut art = Vec::new();
    for a in &manifest.art {
        let bytes = std::fs::read(dir.join(&a.file)).map_err(|e| format!("[client] art {}: {}: {e}", a.member, a.file))?;
        art.push(ArtFile { member: a.member.clone(), file: a.file.clone(), sha256: hex(&bytes) });
    }
    Ok(Plan { dir: dir.to_owned(), manifest, entries, migrations, art })
}

fn range_texts(list: &[IdRange]) -> String {
    if list.is_empty() { "none".to_owned() } else { list.iter().map(IdRange::text).collect::<Vec<_>>().join(", ") }
}

// ----------------------------------------------------------------------------------------------------------------------- small reads

type RowMap = Map<String, Value>;

async fn fetch_row(conn: &mut PgConnection, schema: &str, t: &ExtTable, key: &[i64]) -> Result<Option<RowMap>> {
    let filter = t.key_cols.iter().enumerate().map(|(i, c)| format!("{c} = ${}", i + 1)).collect::<Vec<_>>().join(" AND ");
    let mut query = sqlx::query_scalar::<_, Value>(sql(format!("SELECT to_jsonb(t) FROM {schema}.{} t WHERE {filter}", t.table)));
    for k in key {
        query = query.bind(*k);
    }
    Ok(query.fetch_optional(&mut *conn).await?.and_then(|v| v.as_object().cloned()))
}

async fn has_imports(conn: &mut PgConnection, s: &Schemas, part: &str) -> Result<bool> {
    Ok(sqlx::query_scalar(sql(format!("SELECT EXISTS (SELECT 1 FROM {}.imports WHERE part = $1)", s.content))).bind(part).fetch_one(&mut *conn).await?)
}

#[derive(Debug)]
struct Installed {
    version: semver::Version,
    enabled: bool,
}

async fn installed(conn: &mut PgConnection, s: &Schemas, name: &str) -> Result<Option<Installed>> {
    let row: Option<(String, bool)> =
        sqlx::query_as(sql(format!("SELECT version, enabled FROM {}.extensions WHERE name = $1", s.content))).bind(name).fetch_optional(&mut *conn).await?;
    row.map(|(v, enabled)| Ok(Installed { version: semver::Version::parse(&v).with_context(|| format!("installed version {v:?}"))?, enabled })).transpose()
}

fn is_printable(text: &str) -> bool {
    text.bytes().all(|b| (0x20..=0x7E).contains(&b))
}

fn item_ids(rows: &[(String, String)]) -> Vec<i32> {
    rows.iter().filter(|(t, _)| t == "defs.items").filter_map(|(_, k)| k.parse().ok()).collect()
}

fn skill_ids(rows: &[(String, String)]) -> Vec<(i32, i32)> {
    rows.iter()
        .filter(|(t, _)| t == "defs.skills")
        .filter_map(|(_, k)| k.split_once('/').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
        .collect()
}

// ------------------------------------------------------------------------------------------------------------------- computing rows

/// What a rows entry becomes: the row written, whether it changes an original or adds a row.
struct Computed {
    action: &'static str,
    row: RowMap,
    /// The row differs from the original in a column other than `updatedate` (it needs a client row or a reason).
    differs: bool,
}

async fn compute(
    conn: &mut PgConnection,
    s: &Schemas,
    t: &ExtTable,
    e: &Entry,
    prior_row: Option<&Value>,
) -> Result<Computed> {
    let fail = |text: String| anyhow::Error::from(crate::store::Refused(format!("{}: {text}", e.label())));
    let frozen = s.original_of(t.side);
    let original = fetch_row(conn, frozen, t, &e.key).await?;
    let cols = columns_of(conn, s.of(t.side), t.table).await?;
    let source = match &e.copy {
        Some(c) => {
            if e.kind == Kind::Drop {
                return Err(fail("a drop entry changes grade and droprate of an existing entry; it has nothing to copy".into()));
            }
            Some(fetch_row(conn, frozen, t, &c.from_key).await?.ok_or_else(|| fail(format!("copy from {} is not an original row of {}", c.from, t.tbl)))?)
        }
        None => None,
    };
    let (action, mut row): (&'static str, RowMap) = match (&original, &source) {
        (Some(o), _) => ("change", o.clone()),
        (None, Some(src)) => ("add", src.clone()),
        (None, None) => {
            let what = if e.kind == Kind::Drop {
                format!("{} is not an original row; an extension may only change grade and droprate of an original drop entry", e.id)
            } else {
                format!("{} is not an original row of {}; a new row needs `copy = {{ from = \"…\" }}`", e.id, t.tbl)
            };
            return Err(fail(what));
        }
    };
    if action == "add" {
        // The place must exist in the client and be free in the live table.
        if e.kind == Kind::Item {
            ids::check_ceilings(&IdRange { major: e.key[0] / 1000, low: e.key[0] % 1000, high: e.key[0] % 1000 }, false).map_err(fail)?;
        } else if e.kind == Kind::Skill {
            ids::check_ceilings(&IdRange { major: e.key[0], low: e.key[1], high: e.key[1] }, true).map_err(fail)?;
        }
        if fetch_row(conn, s.of(t.side), t, &e.key).await?.is_some() {
            return Err(fail(format!("{} exists in {} already (it is not an original row, so an edit or another extension made it)", e.id, t.tbl)));
        }
        match e.kind {
            Kind::Item => {
                row.insert("itemid".into(), json!(e.key[0]));
                row.insert("type".into(), json!(e.key[0] / 1000));
                row.insert("typeid".into(), json!(e.key[0] % 1000));
            }
            Kind::Skill => {
                row.insert("skillid".into(), json!(e.key[0]));
                row.insert("skilllevel".into(), json!(e.key[1]));
            }
            Kind::Drop => unreachable!("a drop entry always changes an original row"),
        }
        let rowid = match prior_row.and_then(|r| r.get("rowid")) {
            Some(r) => r.clone(),
            None => {
                let next: i64 = sqlx::query_scalar(sql(format!("SELECT coalesce(max(rowid), 0)::bigint + 1 FROM {}.{}", s.of(t.side), t.table)))
                    .fetch_one(&mut *conn)
                    .await?;
                json!(next)
            }
        };
        row.insert("rowid".into(), rowid);
    }
    if let (Some(c), Some(src)) = (&e.copy, &source) {
        let columns: Vec<String> = match &c.columns {
            Some(list) => list.clone(),
            None => cols.iter().filter(|c| !t.locked_cols.contains(&c.as_str()) && c.as_str() != "updatedate").cloned().collect(),
        };
        for column in columns {
            if !cols.contains(&column) {
                return Err(fail(format!("copy: column {column:?} is not a column of {}", t.tbl)));
            }
            if t.locked_cols.contains(&column.as_str()) {
                return Err(fail(format!("copy: column {column:?} is part of the key and cannot be copied")));
            }
            row.insert(column.clone(), src.get(&column).cloned().unwrap_or(Value::Null));
        }
    }
    let editor_target = registry::target("mob_drop").ok();
    for (column, value) in &e.set {
        if !cols.contains(column) {
            return Err(fail(format!("column {column:?} is not a column of {}", t.tbl)));
        }
        if t.locked_cols.contains(&column.as_str()) {
            return Err(fail(format!("column {column:?} is part of the key or the row's own number; the id names the row")));
        }
        let mut value = value.clone();
        if e.kind == Kind::Drop {
            let target = editor_target.expect("the mob_drop target");
            let field = target.field(column).map_err(|m| fail(format!("{m} (an extension may change grade and droprate of a drop entry)")))?;
            let facts = Facts::default();
            let current = row.clone();
            value = field.check(&value, &Ctx { facts: &facts, map: None, current: &current, key: &e.key }).map_err(|m| fail(format!("set {column}: {m}")))?;
        }
        if e.kind == Kind::Item && column == "range" {
            let v = value.as_i64().ok_or_else(|| fail("set range: a whole number".into()))?;
            if !(0..=ids::ITEM_RANGE_MAX).contains(&v) {
                return Err(fail(format!("set range: {v} does not fit the client's byte for the skill an item gives (0 to {})", ids::ITEM_RANGE_MAX)));
            }
        }
        if let Value::String(text) = &value {
            let ok: Result<bool, sqlx::Error> = sqlx::query_scalar("SELECT convert_to($1::text, 'GBK') IS NOT NULL").bind(text).fetch_one(&mut *conn).await;
            if ok.is_err() {
                return Err(fail(format!("set {column}: the text {text:?} cannot be written in GBK, the encoding of the original definitions")));
            }
        }
        row.insert(column.clone(), value);
    }
    let differs = match &original {
        None => true,
        Some(o) => row.iter().any(|(k, v)| k != "updatedate" && o.get(k) != Some(v)),
    };
    Ok(Computed { action, row, differs })
}

// ------------------------------------------------------------------------------------------------------------------------ client rows

struct ClientCtx {
    exists: bool,
    longest: BTreeMap<&'static str, i64>,
}

async fn client_ctx(conn: &mut PgConnection, s: &Schemas) -> Result<ClientCtx> {
    let exists = schema_exists(conn, &s.client).await? && tables_of(conn, &s.client).await?.iter().any(|t| t == "items");
    let mut longest = BTreeMap::new();
    if exists {
        for (key, table, column) in
            [("item name", "items", "name"), ("item description", "items", "description"), ("skill name", "skills", "name"), ("skill description", "skills", "description")]
        {
            let n: i64 = sqlx::query_scalar(sql(format!("SELECT coalesce(max(octet_length({column})), 0)::bigint FROM {}.{table}", s.client))).fetch_one(&mut *conn).await?;
            longest.insert(key, n);
        }
    }
    Ok(ClientCtx { exists, longest })
}

/// Validates one entry's client part and writes it into `content.client_*` / `content.server_only` (since_patch NULL).
async fn write_client(conn: &mut PgConnection, s: &Schemas, ctx: &ClientCtx, extension: &str, e: &Entry, differs: bool, warnings: &mut Vec<String>) -> Result<bool> {
    let fail = |text: String| anyhow::Error::from(crate::store::Refused(format!("{}: {text}", e.label())));
    let c = &s.content;
    if e.kind == Kind::Drop {
        return Ok(false);
    }
    if differs && e.client.is_none() && e.server_only.is_none() {
        return Err(fail("the row differs from the original, so it needs `client = { … }` (or \"keep\") for the client's data patch, or `server_only = \"reason\"` when the client needs nothing of it".into()));
    }
    if let Some(reason) = &e.server_only {
        let (kind, id, level) = if e.kind == Kind::Item { ("item", e.key[0], 0) } else { ("skill", e.key[0], e.key[1]) };
        let owner: Option<String> = sqlx::query_scalar(sql(format!("SELECT extension FROM {c}.server_only WHERE kind = $1 AND id = $2 AND level = $3")))
            .bind(kind)
            .bind(id as i32)
            .bind(level as i16)
            .fetch_optional(&mut *conn)
            .await?;
        if let Some(owner) = owner {
            return Err(fail(format!("the server_only reason for this row belongs to the extension {owner}")));
        }
        sqlx::query(sql(format!("INSERT INTO {c}.server_only (kind, id, level, extension, reason) VALUES ($1, $2, $3, $4, $5)")))
            .bind(kind)
            .bind(id as i32)
            .bind(level as i16)
            .bind(extension)
            .bind(reason)
            .execute(&mut *conn)
            .await?;
        return Ok(false);
    }
    let Some(spec) = &e.client else { return Ok(false) };
    let item = e.kind == Kind::Item;
    let (names, longest_name, longest_description) = if item { ("item", "item name", "item description") } else { ("skill", "skill name", "skill description") };
    let (keep, name, description, look, fields) = match spec {
        ClientSpec::Keep => (true, None, None, None, BTreeMap::new()),
        ClientSpec::Fields { name, description, look, fields } => (false, Some(name.clone()), Some(description.clone()), *look, fields.clone()),
    };
    for (what, text, key) in [("name", &name, longest_name), ("description", &description, longest_description)] {
        let Some(text) = text else { continue };
        if !is_printable(text) {
            return Err(fail(format!("client {what} {text:?} is not printable ASCII (0x20 to 0x7E)")));
        }
        if ctx.exists && text.len() as i64 > ctx.longest[key] {
            return Err(fail(format!("client {what} of {} bytes is longer than the longest {names} {what} in the original ({})", text.len(), ctx.longest[key])));
        }
    }
    if let Some((a, b)) = look {
        if ctx.exists {
            let found: bool = if item {
                sqlx::query_scalar(sql(format!("SELECT EXISTS (SELECT 1 FROM {}.items WHERE grp = $1 AND idx = $2)", s.client))).bind(a as i16).bind(b as i16).fetch_one(&mut *conn).await?
            } else {
                sqlx::query_scalar(sql(format!("SELECT EXISTS (SELECT 1 FROM {}.skills WHERE skill_id = $1 AND slot = $2)", s.client))).bind(a as i16).bind(b as i16).fetch_one(&mut *conn).await?
            };
            if !found {
                return Err(fail(format!("look {a}/{b} is not a record of the client's {names}s")));
            }
        } else {
            ids::check_ceilings(&IdRange { major: a, low: b, high: b }, !item).map_err(|m| fail(format!("look {a}/{b}: {m}")))?;
        }
        if ctx.exists && keep {
            warnings.push(format!("{}: `keep` ships the server's values under the client's own record", e.label()));
        }
    }
    let note = e.note.clone().unwrap_or_else(|| format!("{extension} {}", e.id));
    let field = |n: &str| fields.get(n).copied();
    if item {
        let (ty, id) = ((e.key[0] / 1000) as i16, (e.key[0] % 1000) as i16);
        let owner: Option<String> = sqlx::query_scalar(sql(format!("SELECT extension FROM {c}.client_items WHERE type = $1 AND typeid = $2"))).bind(ty).bind(id).fetch_optional(&mut *conn).await?;
        if let Some(owner) = owner {
            return Err(fail(format!("the client row of item {}/{} belongs to the extension {owner}", ty, id)));
        }
        sqlx::query(sql(format!(
            "INSERT INTO {c}.client_items (type, typeid, extension, keep, name, description, look_type, look_typeid, model, icon, type2, type3, since_patch, note)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NULL, $13)"
        )))
        .bind(ty)
        .bind(id)
        .bind(extension)
        .bind(keep)
        .bind(name)
        .bind(description)
        .bind(look.map(|l| l.0 as i16))
        .bind(look.map(|l| l.1 as i16))
        .bind(field("model").map(|v| v as i16))
        .bind(field("icon").map(|v| v as i16))
        .bind(field("type2").map(|v| v as i16))
        .bind(field("type3").map(|v| v as i16))
        .bind(note)
        .execute(&mut *conn)
        .await
        .map_err(|err| fail(format!("client field out of range: {err}")))?;
    } else {
        let (id, level) = (e.key[0] as i16, e.key[1] as i16);
        let owner: Option<String> =
            sqlx::query_scalar(sql(format!("SELECT extension FROM {c}.client_skills WHERE skill_id = $1 AND level = $2"))).bind(id).bind(level).fetch_optional(&mut *conn).await?;
        if let Some(owner) = owner {
            return Err(fail(format!("the client row of skill {id}/{level} belongs to the extension {owner}")));
        }
        sqlx::query(sql(format!(
            "INSERT INTO {c}.client_skills (skill_id, level, extension, keep, name, description, look_skill, look_level, icon, animation, toggle_type, sound, since_patch, note)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NULL, $13)"
        )))
        .bind(id)
        .bind(level)
        .bind(extension)
        .bind(keep)
        .bind(name)
        .bind(description)
        .bind(look.map(|l| l.0 as i16))
        .bind(look.map(|l| l.1 as i16))
        .bind(field("icon").map(|v| v as i32))
        .bind(field("animation").map(|v| v as i32))
        .bind(field("toggle_type").map(|v| v as i16))
        .bind(field("sound").map(|v| v as i32))
        .bind(note)
        .execute(&mut *conn)
        .await
        .map_err(|err| fail(format!("client field out of range: {err}")))?;
    }
    Ok(true)
}

// ------------------------------------------------------------------------------------------------------------------- shared checks

/// Restores rows an extension owned: a changed row back to the frozen original, an added row deleted. Takes their `ext_rows` too.
async fn restore_rows(conn: &mut PgConnection, s: &Schemas, extension: &str, rows: &[(String, String, String)]) -> Result<()> {
    for (tbl, key, action) in rows {
        let t = super::tables::find(tbl).with_context(|| format!("ext_rows holds {tbl}, which no extension may touch"))?;
        let numbers = super::tables::parse_key(key).map_err(|e| anyhow::anyhow!(e))?;
        let (live, frozen) = (format!("{}.{}", s.of(t.side), t.table), format!("{}.{}", s.original_of(t.side), t.table));
        let filter = t.key_cols.iter().zip(&numbers).map(|(c, n)| format!("{c} = {n}")).collect::<Vec<_>>().join(" AND ");
        if action == "add" {
            sqlx::query(sql(format!("DELETE FROM {live} WHERE {filter}"))).execute(&mut *conn).await?;
        } else {
            let cols = columns_of(conn, s.of(t.side), t.table).await?;
            let set = cols.iter().map(|c| format!("{c} = o.{c}")).collect::<Vec<_>>().join(", ");
            let on = t.key_cols.iter().map(|c| format!("t.{c} = o.{c}")).collect::<Vec<_>>().join(" AND ");
            let done = sqlx::query(sql(format!(
                "UPDATE {live} t SET {set} FROM {frozen} o WHERE {on} AND {}",
                t.key_cols.iter().zip(&numbers).map(|(c, n)| format!("t.{c} = {n}")).collect::<Vec<_>>().join(" AND ")
            )))
            .execute(&mut *conn)
            .await?
            .rows_affected();
            if done != 1 {
                bail!("restoring {tbl} {key}: the original row or the live row is missing ({done} rows updated)");
            }
        }
        sqlx::query(sql(format!("DELETE FROM {}.ext_rows WHERE tbl = $1 AND key = $2 AND extension = $3", s.content)))
            .bind(tbl)
            .bind(key)
            .bind(extension)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// Holders and references of the rows, as lines; the first list can be forced away, the second cannot.
async fn blockers(conn: &mut PgConnection, s: &Schemas, rows: &[(String, String)]) -> Result<(Vec<String>, Vec<String>)> {
    let (items, skills) = (item_ids(rows), skill_ids(rows));
    let held = refs::holders(conn, &items, &skills).await?;
    let referenced = refs::references(conn, s, &items, &skills, rows).await?;
    Ok((held, referenced))
}

fn too_many(lines: &[String]) -> String {
    let shown: Vec<&String> = lines.iter().take(40).collect();
    let mut text = shown.iter().map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n");
    if lines.len() > shown.len() {
        text.push_str(&format!("\n  … and {} more", lines.len() - shown.len()));
    }
    text
}

/// After rows changed: the `defs` marker goes and the edits are written again on top (originals → extensions → edits).
async fn rewrite_edits(conn: &mut PgConnection, s: &Schemas) -> Result<Vec<String>> {
    sqlx::query(sql(format!("DROP TABLE IF EXISTS {}.edits_applied", s.defs))).execute(&mut *conn).await?;
    let mut report = apply::Report::default();
    apply::apply_side(conn, s, Side::Defs, &mut report).await?;
    Ok(report
        .problems
        .iter()
        .map(|p| {
            let at = format!("{} {}{}", p.target, p.key, p.field.as_ref().map(|f| format!(" {f}")).unwrap_or_default());
            format!("PROBLEM {at}{}: {}", p.change_set.map(|c| format!(" (change set #{c})")).unwrap_or_default(), p.text)
        })
        .collect())
}

async fn log_entry(conn: &mut PgConnection, s: &Schemas, extension: &str, action: &str, detail: Value) -> Result<()> {
    sqlx::query(sql(format!("INSERT INTO {}.ext_log (extension, action, detail) VALUES ($1, $2, $3)", s.content)))
        .bind(extension)
        .bind(action)
        .bind(detail)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

async fn all_hashes(conn: &mut PgConnection, s: &Schemas) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for schema in [&s.defs, &s.world, &s.client] {
        if !schema_exists(conn, schema).await? {
            continue;
        }
        for table in tables_of(conn, schema).await? {
            let (rows, md5) = table_hash(conn, schema, &table).await?;
            out.push(format!("{schema}.{table}|{rows}|{md5}"));
        }
    }
    Ok(out)
}

// ----------------------------------------------------------------------------------------------------------------- install / upgrade

struct Prior {
    /// `(tbl, key, action, row)` as installed.
    rows: Vec<(String, String, String, Value)>,
    /// `since_patch` and a description of the content, per client row (`item type/typeid`, `skill id/level`).
    client: BTreeMap<String, (Option<i32>, String)>,
    migrations: BTreeMap<i32, String>,
}

async fn read_prior(conn: &mut PgConnection, s: &Schemas, name: &str) -> Result<Prior> {
    let c = &s.content;
    let rows = sqlx::query_as(sql(format!("SELECT tbl, key, action, row FROM {c}.ext_rows WHERE extension = $1 ORDER BY seq"))).bind(name).fetch_all(&mut *conn).await?;
    let mut client = BTreeMap::new();
    let items: Vec<(i16, i16, Option<i32>, String)> = sqlx::query_as(sql(format!(
        "SELECT type, typeid, since_patch, concat_ws('|', keep, name, description, look_type, look_typeid, model, icon, type2, type3, note) FROM {c}.client_items WHERE extension = $1"
    )))
    .bind(name)
    .fetch_all(&mut *conn)
    .await?;
    for (a, b, since, content) in items {
        client.insert(format!("item {a}/{b}"), (since, content));
    }
    let skills: Vec<(i16, i16, Option<i32>, String)> = sqlx::query_as(sql(format!(
        "SELECT skill_id, level, since_patch, concat_ws('|', keep, name, description, look_skill, look_level, icon, animation, toggle_type, sound, note) FROM {c}.client_skills WHERE extension = $1"
    )))
    .bind(name)
    .fetch_all(&mut *conn)
    .await?;
    for (a, b, since, content) in skills {
        client.insert(format!("skill {a}/{b}"), (since, content));
    }
    let migrations: Vec<(i32, String)> = sqlx::query_as(sql(format!("SELECT version, sha256 FROM {c}.ext_migrations WHERE extension = $1"))).bind(name).fetch_all(&mut *conn).await?;
    Ok(Prior { rows, client, migrations: migrations.into_iter().collect() })
}

/// The install and the upgrade, inside the caller's transaction (the caller holds the lock). Returns the lines to print.
async fn apply_plan(conn: &mut PgConnection, s: &Schemas, plan: &Plan, prior: Option<Prior>) -> Result<Vec<String>> {
    let m = &plan.manifest;
    let c = &s.content;
    let name = m.name.as_str();
    let mut out = Vec::new();
    let mut warnings = Vec::new();

    // Settings: every one exists and passes the server's parser; no other enabled extension sets it.
    for (key, value) in &m.settings {
        game_server::config::check_setting(key, value).map_err(|e| crate::store::Refused(format!("extension.toml [settings] {key}: {e:#}")))?;
        let other: Option<String> = sqlx::query_scalar(sql(format!(
            "SELECT t.extension FROM {c}.ext_settings t JOIN {c}.extensions e ON e.name = t.extension WHERE t.name = $1 AND e.enabled AND t.extension <> $2"
        )))
        .bind(key)
        .bind(name)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(other) = other {
            return refuse(format!("the setting {key} is set by the enabled extension {other} already; disable it or drop the setting from {name}"));
        }
    }
    // Ranges against the other installed extensions, enabled or not.
    for (kind, list) in [("item", &m.items), ("skill", &m.skills)] {
        for r in list {
            let clash: Option<(String, i32, i32, i32)> = sqlx::query_as(sql(format!(
                "SELECT extension, major, minor_low, minor_high FROM {c}.ext_ranges
                 WHERE kind = $1 AND extension <> $2 AND major = $3 AND minor_low <= $5 AND $4 <= minor_high"
            )))
            .bind(kind)
            .bind(name)
            .bind(r.major as i32)
            .bind(r.low as i32)
            .bind(r.high as i32)
            .fetch_optional(&mut *conn)
            .await?;
            if let Some((other, major, low, high)) = clash {
                return refuse(format!("the {kind} ids {} overlap {major}/{low}-{high} of the extension {other}", r.text()));
            }
        }
    }
    // Keys another extension owns.
    let owners: Vec<(String, String, String)> = sqlx::query_as(sql(format!("SELECT tbl, key, extension FROM {c}.ext_rows WHERE extension <> $1"))).bind(name).fetch_all(&mut *conn).await?;
    for e in &plan.entries {
        let (tbl, key) = (e.kind.table().tbl, e.key_text());
        if let Some((_, _, other)) = owners.iter().find(|(t, k, _)| t == tbl && *k == key) {
            return refuse(format!("{}: {tbl} {key} is owned by the extension {other}", e.label()));
        }
    }

    // Upgrade: the keys no longer listed go back (checked as a remove is).
    let new_keys: BTreeSet<(String, String)> = plan.entries.iter().map(|e| (e.kind.table().tbl.to_owned(), e.key_text())).collect();
    if let Some(prior) = &prior {
        if let Some(applied) = prior.migrations.iter().find(|(v, sha)| plan.migrations.iter().find(|m| m.version == **v).is_none_or(|m| m.sha256 != **sha)) {
            return refuse(format!("migration {:04} was applied as it was before and the folder has it changed or gone; an applied migration is never rewritten", applied.0));
        }
        let dropped: Vec<(String, String, String)> =
            prior.rows.iter().filter(|(t, k, _, _)| !new_keys.contains(&(t.clone(), k.clone()))).map(|(t, k, a, _)| (t.clone(), k.clone(), a.clone())).collect();
        let keys: Vec<(String, String)> = dropped.iter().map(|(t, k, _)| (t.clone(), k.clone())).collect();
        let (held, referenced) = blockers(conn, s, &keys).await?;
        if !held.is_empty() || !referenced.is_empty() {
            let mut text = format!("{name} {} no longer lists rows that players hold or other data points at:\n", m.version);
            if !held.is_empty() {
                text.push_str(&format!("held:\n{}\n", too_many(&held)));
            }
            if !referenced.is_empty() {
                text.push_str(&format!("referenced:\n{}\n", too_many(&referenced)));
            }
            return refuse(text.trim_end().to_owned());
        }
        restore_rows(conn, s, name, &dropped).await?;
        out.push(format!("{} rows no longer listed were restored", dropped.len()));
    }

    // The extension's own records: replaced.
    let manifest_json = m.json.clone();
    if prior.is_some() {
        sqlx::query(sql(format!("UPDATE {c}.extensions SET version = $2, title = $3, description = $4, manifest = $5, upgraded_at = now() WHERE name = $1")))
            .bind(name)
            .bind(m.version.to_string())
            .bind(&m.title)
            .bind(&m.description)
            .bind(&manifest_json)
            .execute(&mut *conn)
            .await?;
        for table in ["ext_ranges", "ext_settings", "ext_client"] {
            sqlx::query(sql(format!("DELETE FROM {c}.{table} WHERE extension = $1"))).bind(name).execute(&mut *conn).await?;
        }
    } else {
        sqlx::query(sql(format!("INSERT INTO {c}.extensions (name, version, title, description, manifest) VALUES ($1, $2, $3, $4, $5)")))
            .bind(name)
            .bind(m.version.to_string())
            .bind(&m.title)
            .bind(&m.description)
            .bind(&manifest_json)
            .execute(&mut *conn)
            .await?;
    }
    for (kind, list) in [("item", &m.items), ("skill", &m.skills)] {
        for r in list {
            sqlx::query(sql(format!("INSERT INTO {c}.ext_ranges (extension, kind, major, minor_low, minor_high) VALUES ($1, $2, $3, $4, $5)")))
                .bind(name)
                .bind(kind)
                .bind(r.major as i32)
                .bind(r.low as i32)
                .bind(r.high as i32)
                .execute(&mut *conn)
                .await?;
        }
    }
    for (key, value) in &m.settings {
        sqlx::query(sql(format!("INSERT INTO {c}.ext_settings (extension, name, value) VALUES ($1, $2, $3)"))).bind(name).bind(key).bind(value).execute(&mut *conn).await?;
    }
    for f in &m.features {
        sqlx::query(sql(format!("INSERT INTO {c}.ext_client (extension, kind, name) VALUES ($1, 'feature', $2)"))).bind(name).bind(f).execute(&mut *conn).await?;
    }
    for a in &plan.art {
        sqlx::query(sql(format!("INSERT INTO {c}.ext_client (extension, kind, name, file, sha256) VALUES ($1, 'art', $2, $3, $4)")))
            .bind(name)
            .bind(&a.member)
            .bind(&a.file)
            .bind(&a.sha256)
            .execute(&mut *conn)
            .await?;
    }

    // Migrations (5.4): new ones only, in the extension's own schema; the content tables must come out as they went in.
    let done = prior.as_ref().map(|p| p.migrations.keys().copied().max().unwrap_or(0)).unwrap_or(0);
    let pending: Vec<&Migration> = plan.migrations.iter().filter(|mg| mg.version > done).collect();
    if !pending.is_empty() {
        let schema = m.schema(&s.ext_prefix);
        let before = all_hashes(conn, s).await?;
        sqlx::query(sql(format!("CREATE SCHEMA IF NOT EXISTS {schema}"))).execute(&mut *conn).await?;
        let path: String = sqlx::query_scalar("SELECT current_setting('search_path')").fetch_one(&mut *conn).await?;
        for mg in &pending {
            sqlx::query(sql(format!("SET LOCAL search_path TO {schema}"))).execute(&mut *conn).await?;
            let ran = sqlx::raw_sql(sql(mg.sql.clone())).execute(&mut *conn).await;
            sqlx::query("SELECT set_config('search_path', $1, true)").bind(&path).execute(&mut *conn).await?;
            ran.map_err(|e| crate::store::Refused(format!("migrations/{}.sql: {e}", mg.name)))?;
            sqlx::query(sql(format!("INSERT INTO {c}.ext_migrations (extension, version, name, sha256) VALUES ($1, $2, $3, $4)")))
                .bind(name)
                .bind(mg.version)
                .bind(&mg.name)
                .bind(&mg.sha256)
                .execute(&mut *conn)
                .await?;
        }
        if all_hashes(conn, s).await? != before {
            return refuse("the migrations changed rows of defs, world or client; an extension's migrations may only create and fill tables of its own schema".to_owned());
        }
        out.push(format!("ran {} migration(s) in the schema {schema}", pending.len()));
    }

    // The rows.
    let client = client_ctx(conn, s).await?;
    let (mut changed, mut added, mut client_rows) = (0, 0, 0);
    let mut shown = Vec::new();
    // Client rows are rewritten wholesale; the old ones' since_patch is looked up first.
    for table in ["client_items", "client_skills"] {
        sqlx::query(sql(format!("DELETE FROM {c}.{table} WHERE extension = $1"))).bind(name).execute(&mut *conn).await?;
    }
    sqlx::query(sql(format!("DELETE FROM {c}.server_only WHERE extension = $1"))).bind(name).execute(&mut *conn).await?;
    let mut new_client_keys = Vec::new();
    for (seq, e) in plan.entries.iter().enumerate() {
        let t = e.kind.table();
        let key = e.key_text();
        let before = prior.as_ref().and_then(|p| p.rows.iter().find(|(tbl, k, _, _)| tbl == t.tbl && *k == key));
        let computed = compute(conn, s, t, e, before.map(|b| &b.3)).await?;
        let row = Value::Object(computed.row.clone());
        // A row this extension wrote before is already in the live table: it is rewritten in place.
        let write_as = if before.is_some() { "change" } else { computed.action };
        write_ext_row(conn, s, t, write_as, &row).await.map_err(|err| crate::store::Refused(format!("{}: {err:#}", e.label())))?;
        let written: Value = {
            let filter = t.key_cols.iter().zip(&e.key).map(|(c, n)| format!("{c} = {n}")).collect::<Vec<_>>().join(" AND ");
            sqlx::query_scalar(sql(format!("SELECT to_jsonb(t) FROM {}.{} t WHERE {filter}", s.of(t.side), t.table))).fetch_one(&mut *conn).await?
        };
        if before.is_some() {
            sqlx::query(sql(format!("DELETE FROM {c}.ext_rows WHERE tbl = $1 AND key = $2"))).bind(t.tbl).bind(&key).execute(&mut *conn).await?;
        }
        sqlx::query(sql(format!("INSERT INTO {c}.ext_rows (extension, tbl, key, action, row, seq) VALUES ($1, $2, $3, $4, $5, $6)")))
            .bind(name)
            .bind(t.tbl)
            .bind(&key)
            .bind(computed.action)
            .bind(&written)
            .bind(seq as i32)
            .execute(&mut *conn)
            .await?;
        if computed.action == "add" { added += 1 } else { changed += 1 }
        if write_client(conn, s, &client, name, e, computed.differs, &mut warnings).await? {
            client_rows += 1;
            let label = if e.kind == Kind::Item { format!("item {}/{}", e.key[0] / 1000, e.key[0] % 1000) } else { format!("skill {}/{}", e.key[0], e.key[1]) };
            new_client_keys.push((label, before.map(|b| b.3 == written).unwrap_or(false)));
        }
        shown.push(describe(e, &computed, t));
    }
    // since_patch survives for a client row whose content and server row did not change.
    if let Some(prior) = &prior {
        for (label, server_same) in &new_client_keys {
            let Some((Some(since), old_content)) = prior.client.get(label) else { continue };
            let (a, b) = label.split_once(' ').unwrap_or_default();
            let (x, y) = b.split_once('/').unwrap_or_default();
            let (table, c1, c2) = if a == "item" { ("client_items", "type", "typeid") } else { ("client_skills", "skill_id", "level") };
            let now_content: String = sqlx::query_scalar(sql(format!(
                "SELECT concat_ws('|', {}) FROM {c}.{table} WHERE {c1} = {x} AND {c2} = {y}",
                if a == "item" { "keep, name, description, look_type, look_typeid, model, icon, type2, type3, note" } else { "keep, name, description, look_skill, look_level, icon, animation, toggle_type, sound, note" }
            )))
            .fetch_one(&mut *conn)
            .await?;
            if *server_same && now_content == *old_content {
                sqlx::query(sql(format!("UPDATE {c}.{table} SET since_patch = $1 WHERE {c1} = {x} AND {c2} = {y}"))).bind(*since).execute(&mut *conn).await?;
            }
        }
    }

    // On top: the edits.
    let problems = rewrite_edits(conn, s).await?;
    out.push(format!("{changed} rows changed, {added} rows added, {client_rows} client rows, {} settings", m.settings.len()));
    out.extend(shown);
    out.extend(warnings.into_iter().map(|w| format!("warning: {w}")));
    out.extend(problems);
    Ok(out)
}

fn describe(e: &Entry, c: &Computed, t: &ExtTable) -> String {
    let what = match c.action {
        "add" => "adds",
        _ => "changes",
    };
    let cols: Vec<&str> = e.set.iter().map(|(k, _)| k.as_str()).collect();
    format!("  {} {what} {} {}{}", e.label(), t.tbl, e.key_text(), if cols.is_empty() { String::new() } else { format!(" (set: {})", cols.join(", ")) })
}

fn next_steps(plan: &Plan, wrote_items_or_skills: bool, wrote_drops: bool, client_rows: bool) -> Vec<String> {
    let mut out = Vec::new();
    if wrote_items_or_skills {
        out.push("next: restart the game servers (tools/game-server.sh restart): items, skills and settings are read at start".to_owned());
    } else if wrote_drops {
        out.push("next: `console <server id> reload` picks up the drop entries".to_owned());
    }
    if client_rows {
        out.push("next: run tools/sdata/client_patch.py check, then generate a client patch".to_owned());
        out.push("      the players' clients show these items as ??? until they have a patch that carries them".to_owned());
    }
    if !plan.manifest.features.is_empty() {
        out.push(format!("the extension needs these fixes.dll features in the client: {}", plan.manifest.features.join(", ")));
    }
    out
}

impl Editor {
    fn read_plan(dir: &Path) -> Result<Plan> {
        read_folder(dir).or_else(refuse)
    }

    async fn require_defs_import(&self, conn: &mut PgConnection) -> Result<()> {
        if !has_imports(conn, &self.s, "defs").await? {
            return refuse("defs has no import record: run tools/setup.sh again with the original files first (a database from before 0.3.0: docs/plans/ext-1-foundation.md decision 11)".to_owned());
        }
        Ok(())
    }

    fn report_lines(plan: &Plan, mut lines: Vec<String>) -> Vec<String> {
        let wrote = plan.entries.iter().any(|e| matches!(e.kind, Kind::Item | Kind::Skill));
        let drops = plan.entries.iter().any(|e| e.kind == Kind::Drop);
        let client = plan.entries.iter().any(|e| e.client.is_some() || e.server_only.is_some());
        lines.extend(next_steps(plan, wrote, drops, client));
        lines
    }

    /// `ext install [--skip-installed] <folder>`.
    pub async fn ext_install(&self, dir: &Path, skip_installed: bool) -> Result<Vec<String>> {
        let plan = Self::read_plan(dir)?;
        let mut tx = self.begin_locked().await?;
        self.require_defs_import(&mut tx).await?;
        let name = plan.manifest.name.clone();
        if let Some(have) = installed(&mut tx, &self.s, &name).await? {
            if skip_installed && have.version == plan.manifest.version {
                tx.rollback().await?;
                return Ok(vec![format!("{name} {} is already installed; nothing to do", have.version)]);
            }
            return refuse(format!(
                "{name} {} is installed already; to move to {} use `data-editor ext upgrade {}` (a higher version), or remove it first",
                have.version,
                plan.manifest.version,
                dir.display()
            ));
        }
        let mut lines = vec![format!("installing {name} {}", plan.manifest.version)];
        lines.extend(apply_plan(&mut tx, &self.s, &plan, None).await?);
        log_entry(&mut tx, &self.s, &name, "install", json!({"version": plan.manifest.version.to_string(), "rows": plan.entries.len()})).await?;
        tx.commit().await?;
        Ok(Self::report_lines(&plan, lines))
    }

    /// `ext check <folder>`: everything an install checks, nothing written (rolled back).
    pub async fn ext_check(&self, dir: &Path) -> Result<Vec<String>> {
        let plan = Self::read_plan(dir)?;
        let mut tx = self.begin_locked().await?;
        self.require_defs_import(&mut tx).await?;
        let name = plan.manifest.name.clone();
        let mut lines = Vec::new();
        let prior = match installed(&mut tx, &self.s, &name).await? {
            Some(have) if have.version >= plan.manifest.version => {
                tx.rollback().await?;
                return refuse(format!("{name} {} is installed; a check needs a higher version to compare (an upgrade) or an extension that is not installed", have.version));
            }
            Some(have) => {
                lines.push(format!("{name} {} is installed: checking the upgrade to {}", have.version, plan.manifest.version));
                Some(read_prior(&mut tx, &self.s, &name).await?)
            }
            None => {
                lines.push(format!("checking the install of {name} {}", plan.manifest.version));
                None
            }
        };
        let result = apply_plan(&mut tx, &self.s, &plan, prior).await;
        tx.rollback().await?;
        lines.extend(result?);
        lines.push("nothing was written".to_owned());
        Ok(Self::report_lines(&plan, lines))
    }

    /// `ext upgrade <folder>`.
    pub async fn ext_upgrade(&self, dir: &Path) -> Result<Vec<String>> {
        let plan = Self::read_plan(dir)?;
        let mut tx = self.begin_locked().await?;
        self.require_defs_import(&mut tx).await?;
        let name = plan.manifest.name.clone();
        let Some(have) = installed(&mut tx, &self.s, &name).await? else {
            return refuse(format!("{name} is not installed; use `data-editor ext install {}`", dir.display()));
        };
        if plan.manifest.version <= have.version {
            return refuse(format!("{name} {} is installed; an upgrade needs a higher version than that (the folder has {})", have.version, plan.manifest.version));
        }
        let prior = read_prior(&mut tx, &self.s, &name).await?;
        let mut lines = vec![format!("upgrading {name} {} -> {}", have.version, plan.manifest.version)];
        lines.extend(apply_plan(&mut tx, &self.s, &plan, Some(prior)).await?);
        log_entry(&mut tx, &self.s, &name, "upgrade", json!({"from": have.version.to_string(), "to": plan.manifest.version.to_string()})).await?;
        tx.commit().await?;
        Ok(Self::report_lines(&plan, lines))
    }

    /// `ext remove <name> [--force]`. `confirm` is asked, with the lines of what `--force` deletes, before it deletes anything.
    pub async fn ext_remove(&self, name: &str, force: bool, confirm: impl FnOnce(&[String]) -> Result<bool>) -> Result<Vec<String>> {
        let s = &self.s;
        let c = &s.content;
        let mut tx = self.begin_locked().await?;
        if installed(&mut tx, s, name).await?.is_none() {
            return refuse(format!("{name} is not installed"));
        }
        let rows: Vec<(String, String, String)> = sqlx::query_as(sql(format!("SELECT tbl, key, action FROM {c}.ext_rows WHERE extension = $1 ORDER BY seq DESC"))).bind(name).fetch_all(&mut *tx).await?;
        let keys: Vec<(String, String)> = rows.iter().map(|(t, k, _)| (t.clone(), k.clone())).collect();
        let (held, referenced) = blockers(&mut tx, s, &keys).await?;
        if !referenced.is_empty() {
            let mut text = format!("other data points at the rows of {name}:\n{}\nchange those rows or undo the edits first; --force does not clear references", too_many(&referenced));
            if !held.is_empty() {
                text.push_str(&format!("\n(players also hold:\n{}\n)", too_many(&held)));
            }
            return refuse(text);
        }
        let mut lines = Vec::new();
        if !held.is_empty() {
            if !force {
                return refuse(format!("players hold the items and skills of {name}:\n{}\nremove them from the players first, or use --force to delete them (logged)", too_many(&held)));
            }
            let online = refs::players_online(&mut tx).await?;
            if online > 0 {
                return refuse(format!("the game servers report {online} players online; an online player's memory would write a deleted item back. Stop the game servers first"));
            }
            if !confirm(&held)? {
                return refuse("not confirmed; nothing was deleted".to_owned());
            }
            let deleted = refs::force_delete(&mut tx, &item_ids(&keys), &skill_ids(&keys)).await?;
            for d in &deleted {
                log_entry(&mut tx, s, name, "force-delete", d.clone()).await?;
            }
            lines.push(format!("deleted {} held rows (logged)", deleted.len()));
        }
        let shipped: i64 = sqlx::query_scalar(sql(format!(
            "SELECT (SELECT count(*) FROM {c}.client_items WHERE extension = $1 AND since_patch IS NOT NULL) + (SELECT count(*) FROM {c}.client_skills WHERE extension = $1 AND since_patch IS NOT NULL)"
        )))
        .bind(name)
        .fetch_one(&mut *tx)
        .await?;
        let version: String = sqlx::query_scalar(sql(format!("SELECT version FROM {c}.extensions WHERE name = $1"))).bind(name).fetch_one(&mut *tx).await?;
        // Restore in reverse order of writing, then take the extension's records out.
        restore_rows(&mut tx, s, name, &rows).await?;
        for table in ["client_items", "client_skills", "server_only"] {
            sqlx::query(sql(format!("DELETE FROM {c}.{table} WHERE extension = $1"))).bind(name).execute(&mut *tx).await?;
        }
        let migrations: i64 = sqlx::query_scalar(sql(format!("SELECT count(*) FROM {c}.ext_migrations WHERE extension = $1"))).bind(name).fetch_one(&mut *tx).await?;
        if migrations > 0 {
            let schema = format!("{}{}", s.ext_prefix, name.replace('-', "_"));
            sqlx::query(sql(format!("DROP SCHEMA IF EXISTS {schema} CASCADE"))).execute(&mut *tx).await?;
            lines.push(format!("dropped the schema {schema}"));
        }
        sqlx::query(sql(format!("DELETE FROM {c}.extensions WHERE name = $1"))).bind(name).execute(&mut *tx).await?;
        let problems = rewrite_edits(&mut tx, s).await?;
        log_entry(&mut tx, s, name, "remove", json!({"version": version, "rows": rows.iter().map(|(t, k, _)| format!("{t} {k}")).collect::<Vec<_>>(), "shipped_client_rows": shipped})).await?;
        tx.commit().await?;
        lines.insert(0, format!("removed {name} {version}: {} rows restored", rows.len()));
        lines.extend(problems);
        lines.push("next: restart the game servers".to_owned());
        if shipped > 0 {
            lines.push(format!("a client patch is due: {shipped} client rows were in an earlier patch and are gone now"));
        }
        Ok(lines)
    }

    /// `ext enable` / `ext disable`.
    pub async fn ext_set_enabled(&self, name: &str, enabled: bool) -> Result<Vec<String>> {
        let s = &self.s;
        let c = &s.content;
        let mut tx = self.begin_locked().await?;
        let Some(have) = installed(&mut tx, s, name).await? else {
            return refuse(format!("{name} is not installed"));
        };
        if have.enabled == enabled {
            return Ok(vec![format!("{name} is {} already", if enabled { "enabled" } else { "disabled" })]);
        }
        if enabled {
            let clash: Option<(String, String)> = sqlx::query_as(sql(format!(
                "SELECT t.name, o.extension FROM {c}.ext_settings t JOIN {c}.ext_settings o ON o.name = t.name AND o.extension <> t.extension
                 JOIN {c}.extensions e ON e.name = o.extension AND e.enabled WHERE t.extension = $1 LIMIT 1"
            )))
            .bind(name)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some((setting, other)) = clash {
                return refuse(format!("the setting {setting} is set by the enabled extension {other}; disable it first"));
            }
        }
        sqlx::query(sql(format!("UPDATE {c}.extensions SET enabled = $2 WHERE name = $1"))).bind(name).bind(enabled).execute(&mut *tx).await?;
        log_entry(&mut tx, s, name, if enabled { "enable" } else { "disable" }, json!({})).await?;
        tx.commit().await?;
        Ok(vec![
            format!("{name} {}: its settings defaults {}; its rows stay", if enabled { "enabled" } else { "disabled" }, if enabled { "count again" } else { "stop counting" }),
            "next: restart the game servers".to_owned(),
        ])
    }

    /// `ext list`.
    pub async fn ext_list(&self) -> Result<Vec<String>> {
        let c = &self.s.content;
        let rows: Vec<(String, String, bool, i64, i64, i64)> = sqlx::query_as(sql(format!(
            "SELECT e.name, e.version, e.enabled, (SELECT count(*) FROM {c}.ext_rows r WHERE r.extension = e.name),
                    (SELECT count(*) FROM {c}.client_items i WHERE i.extension = e.name) + (SELECT count(*) FROM {c}.client_skills k WHERE k.extension = e.name),
                    (SELECT count(*) FROM {c}.ext_settings t WHERE t.extension = e.name) FROM {c}.extensions e ORDER BY e.install_seq"
        )))
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Ok(vec!["no extensions installed".to_owned()]);
        }
        let mut out = vec![format!("{:<24} {:<10} {:<9} {:>5} {:>7} {:>9}", "name", "version", "state", "rows", "client", "settings")];
        for (name, version, enabled, r, cl, se) in rows {
            out.push(format!("{name:<24} {version:<10} {:<9} {r:>5} {cl:>7} {se:>9}", if enabled { "enabled" } else { "disabled" }));
        }
        Ok(out)
    }

    /// `ext show <name>`.
    pub async fn ext_show(&self, name: &str) -> Result<Vec<String>> {
        let s = &self.s;
        let c = &s.content;
        let mut conn = self.pool.acquire().await?;
        let row: Option<(String, String, String, bool, Value)> =
            sqlx::query_as(sql(format!("SELECT version, title, description, enabled, manifest FROM {c}.extensions WHERE name = $1"))).bind(name).fetch_optional(&mut *conn).await?;
        let Some((version, title, description, enabled, manifest)) = row else {
            return refuse(format!("{name} is not installed"));
        };
        let mut out = vec![format!("{name} {version}: {title} ({})", if enabled { "enabled" } else { "disabled" }), format!("  {description}")];
        let ranges: Vec<(String, i32, i32, i32)> = sqlx::query_as(sql(format!("SELECT kind, major, minor_low, minor_high FROM {c}.ext_ranges WHERE extension = $1 ORDER BY kind, major, minor_low"))).bind(name).fetch_all(&mut *conn).await?;
        for (kind, major, low, high) in ranges {
            out.push(format!("  ids: {kind} {}", IdRange { major: major.into(), low: low.into(), high: high.into() }.text()));
        }
        let settings: Vec<(String, String)> = sqlx::query_as(sql(format!("SELECT name, value FROM {c}.ext_settings WHERE extension = $1 ORDER BY name"))).bind(name).fetch_all(&mut *conn).await?;
        for (k, v) in settings {
            out.push(format!("  setting {k} = {v:?}"));
        }
        let rows: Vec<(String, String, String, Value)> = sqlx::query_as(sql(format!("SELECT tbl, key, action, row FROM {c}.ext_rows WHERE extension = $1 ORDER BY seq"))).bind(name).fetch_all(&mut *conn).await?;
        for (tbl, key, action, row) in rows {
            let t = super::tables::find(&tbl);
            let original = match (&t, super::tables::parse_key(&key)) {
                (Some(t), Ok(k)) if action == "change" => fetch_row(&mut conn, s.original_of(t.side), t, &k).await?,
                _ => None,
            };
            let diff: Vec<String> = match (&original, row.as_object()) {
                (Some(o), Some(r)) => r.iter().filter(|(k, v)| o.get(k.as_str()) != Some(*v)).map(|(k, v)| format!("{k}: {} -> {v}", o.get(k.as_str()).unwrap_or(&Value::Null))).collect(),
                _ => Vec::new(),
            };
            out.push(format!("  row {action} {tbl} {key}{}", if diff.is_empty() { String::new() } else { format!(": {}", diff.join("; ")) }));
        }
        let client: Vec<(String, String)> = sqlx::query_as(sql(format!(
            "SELECT 'item ' || type || '/' || typeid, coalesce(name, '(keep)') || ' since patch ' || coalesce(since_patch::text, '-') FROM {c}.client_items WHERE extension = $1
             UNION ALL SELECT 'skill ' || skill_id || '/' || level, coalesce(name, '(keep)') || ' since patch ' || coalesce(since_patch::text, '-') FROM {c}.client_skills WHERE extension = $1
             UNION ALL SELECT 'server-only ' || kind || ' ' || id || '/' || level, reason FROM {c}.server_only WHERE extension = $1 ORDER BY 1"
        )))
        .bind(name)
        .fetch_all(&mut *conn)
        .await?;
        for (what, text) in client {
            out.push(format!("  client {what}: {text}"));
        }
        out.push(format!("  manifest: {}", manifest));
        Ok(out)
    }
}
