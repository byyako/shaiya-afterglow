//! The draft, Apply, Undo and the history (`docs/plans/done/data-editing.md` sections 3 and 4). Every change goes through [`apply_line`]: it
//! checks the registry's rules against the row as it is, writes the table, keeps `edits.fields` / `edits.rows` (what survives a
//! reload) and logs the change. Apply and Undo are one transaction each, under an exclusive lock on `edits.change_sets`.

use std::collections::BTreeSet;
use std::fmt;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{AssertSqlSafe, PgConnection, PgPool};

use crate::apply::{Problem, ensure_applied};
use crate::registry::{self, Check, Ctx, Effect, Facts, MapFrom, Schemas, Target, canon, format_key};

/// A refusal the operator can act on (the registry's reason, a missing row, a conflict), as opposed to a database failure.
#[derive(Debug)]
pub struct Refused(pub String);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

pub fn refuse<T>(message: impl Into<String>) -> Result<T> {
    Err(anyhow::Error::new(Refused(message.into())))
}

pub fn user<T>(check: Check<T>) -> Result<T> {
    check.map_err(|m| anyhow::Error::new(Refused(m)))
}

/// One change. `Unset` sets a field back to its original value; `Restore` is only made by Undo.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Set { field: String, value: Value },
    Unset { field: String },
    Add { fields: Value },
    Remove,
    Restore { snapshot: Value },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub target: String,
    pub key: String,
    pub op: Op,
}

/// A row as it was when it was removed (or as Undo found it): its editable fields, the edits in force on it and its child rows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub target: String,
    pub key: String,
    /// The row was added by the edits (it is not in the original files).
    pub added: bool,
    pub row: Map<String, Value>,
    pub edits: Vec<FieldEdit>,
    pub children: Vec<Snapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldEdit {
    pub field: String,
    pub value: Value,
    pub original: Option<Value>,
}

impl Snapshot {
    /// This node and its children.
    pub fn nodes(&self) -> impl Iterator<Item = &Snapshot> {
        std::iter::once(self).chain(self.children.iter())
    }
}

/// The result of an Apply or Undo.
#[derive(Clone, Debug, Serialize)]
pub struct Applied {
    pub change_set: i32,
    pub changes: usize,
    pub effects: BTreeSet<Effect>,
    /// What writing the edits into freshly loaded tables found before this change (see `apply::ensure_applied`).
    pub problems: Vec<Problem>,
}

impl Serialize for Effect {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

/// The state of one Apply: the change set, the log position, the facts the rules need and the effects so far.
pub struct Run<'a> {
    pub s: &'a Schemas,
    pub cs: i32,
    pub seq: i32,
    pub facts: Facts,
    pub effects: BTreeSet<Effect>,
}

impl<'a> Run<'a> {
    pub async fn new(conn: &mut PgConnection, s: &'a Schemas, cs: i32) -> Result<Self> {
        Ok(Self { s, cs, seq: 0, facts: load_facts(conn, s).await?, effects: BTreeSet::new() })
    }
}

fn sql(text: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(text)
}

/// Map sizes and the monsters that can spawn (`MobDefs::get`: id, HP and level not 0).
pub async fn load_facts(conn: &mut PgConnection, s: &Schemas) -> Result<Facts> {
    let maps: Vec<(i64, i64)> = sqlx::query_as(sql(format!("SELECT id::bigint, size::bigint FROM {}.maps", s.world)))
        .fetch_all(&mut *conn)
        .await
        .context("reading world.maps")?;
    let mobs: Vec<i64> =
        sqlx::query_scalar(sql(format!("SELECT mobid::bigint FROM {}.mobs WHERE mobid <> 0 AND hp <> 0 AND level <> 0", s.defs)))
            .fetch_all(&mut *conn)
            .await
            .context("reading defs.mobs")?;
    Ok(Facts { map_sizes: maps.into_iter().collect(), spawnable: mobs.into_iter().collect() })
}

/// A row of a target as JSON, or `None`.
pub async fn read_row(conn: &mut PgConnection, s: &Schemas, t: &Target, key: &[i64]) -> Result<Option<Map<String, Value>>> {
    let query = sql(format!("SELECT to_jsonb(t) FROM {} t WHERE {}", s.table(t), t.where_sql(&s.world, 1)));
    let mut q = sqlx::query_scalar::<_, Value>(query);
    for k in key {
        q = q.bind(*k);
    }
    Ok(q.fetch_optional(&mut *conn).await?.and_then(|v| v.as_object().cloned()))
}

/// The map a row's positions lie in.
async fn row_map(conn: &mut PgConnection, s: &Schemas, t: &Target, key: &[i64], row: &Map<String, Value>) -> Result<Option<i64>> {
    Ok(match t.map_from {
        MapFrom::Key => key.first().copied(),
        MapFrom::Column(column) => row.get(column).and_then(Value::as_i64),
        MapFrom::ToMap => row.get("to_map").and_then(Value::as_i64),
        MapFrom::Parent(parent, at) => {
            let parent = user(registry::target(parent))?;
            read_row(conn, s, parent, &[key[at]]).await?.and_then(|r| r.get("map").and_then(Value::as_i64))
        }
        MapFrom::None => None,
    })
}

/// `'add'` or `'remove'` when the edits added or removed the row.
async fn row_action(conn: &mut PgConnection, s: &Schemas, target: &str, key: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar(sql(format!("SELECT action FROM {}.rows WHERE target = $1 AND key = $2", s.edits)))
        .bind(target)
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?)
}

// One argument per column of the log row it writes; a struct here would only rename them.
#[allow(clippy::too_many_arguments)]
async fn log(
    conn: &mut PgConnection,
    run: &mut Run<'_>,
    target: &str,
    key: &str,
    field: Option<&str>,
    action: &str,
    before: Option<&Value>,
    after: Option<&Value>,
) -> Result<()> {
    run.seq += 1;
    sqlx::query(sql(format!(
        "INSERT INTO {}.log (change_set, seq, target, key, field, action, before, after) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        run.s.edits
    )))
    .bind(run.cs)
    .bind(run.seq)
    .bind(target)
    .bind(key)
    .bind(field)
    .bind(action)
    .bind(before)
    .bind(after)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Carries out one change (inside the caller's transaction) and logs it.
pub async fn apply_line(conn: &mut PgConnection, run: &mut Run<'_>, line: &Line) -> Result<()> {
    let t = user(registry::target(&line.target))?;
    let key = user(t.parse_key(&line.key))?;
    let key_text = format_key(&key);
    let done = match &line.op {
        Op::Set { field, value } => set_field(conn, run, t, &key, &key_text, field, value).await,
        Op::Unset { field } => unset_field(conn, run, t, &key, &key_text, field).await,
        Op::Add { fields } => add_row(conn, run, t, &key, &key_text, fields).await,
        Op::Remove => remove_row(conn, run, t, &key, &key_text).await,
        Op::Restore { snapshot } => restore_row(conn, run, snapshot).await,
    };
    done.map_err(|e| match e.downcast::<Refused>() {
        Ok(Refused(m)) => anyhow::Error::new(Refused(format!("{} {key_text}: {m}", t.name))),
        Err(e) => e.context(format!("{} {key_text}", t.name)),
    })
}

async fn unset_field(conn: &mut PgConnection, run: &mut Run<'_>, t: &Target, key: &[i64], key_text: &str, field: &str) -> Result<()> {
    let stored: Option<Option<Value>> =
        sqlx::query_scalar(sql(format!("SELECT original FROM {}.fields WHERE target = $1 AND key = $2 AND field = $3", run.s.edits)))
            .bind(t.name)
            .bind(key_text)
            .bind(field)
            .fetch_optional(&mut *conn)
            .await?;
    match stored {
        Some(Some(original)) => set_field(conn, run, t, key, key_text, field, &original).await,
        Some(None) => refuse(format!("{field} belongs to a row the edits added: it has no original value")),
        None => refuse(format!("{field} has no edit to undo")),
    }
}

async fn set_field(
    conn: &mut PgConnection,
    run: &mut Run<'_>,
    t: &Target,
    key: &[i64],
    key_text: &str,
    field_name: &str,
    value: &Value,
) -> Result<()> {
    let field = user(t.field(field_name))?;
    let Some(row) = read_row(conn, run.s, t, key).await? else { return refuse("the row does not exist") };
    if field.nullable && row.get(field.name).is_none_or(Value::is_null) {
        return refuse(format!("{} is not set on this row (a boss group without random respawn has no {})", field.name, field.name));
    }
    let map = row_map(conn, run.s, t, key, &row).await?;
    let ctx = Ctx { facts: &run.facts, map, current: &row, key };
    let new = user(field.check(value, &ctx))?;
    let current = canon(row.get(field.name).unwrap_or(&Value::Null));
    if new == current {
        return Ok(());
    }
    let mut candidate = row.clone();
    candidate.insert(field.name.to_owned(), new.clone());
    user(t.check_row(&candidate, Some(field.name)))?;

    let s = run.s;
    let added = row_action(conn, s, t.name, key_text).await?.as_deref() == Some("add");
    let stored: Option<Option<Value>> =
        sqlx::query_scalar(sql(format!("SELECT original FROM {}.fields WHERE target = $1 AND key = $2 AND field = $3", s.edits)))
            .bind(t.name)
            .bind(key_text)
            .bind(field.name)
            .fetch_optional(&mut *conn)
            .await?;
    let original = match (stored, added) {
        (Some(original), _) => original,
        (None, true) => None,
        (None, false) => Some(current.clone()),
    };
    let action = if original.as_ref() == Some(&new) {
        sqlx::query(sql(format!("DELETE FROM {}.fields WHERE target = $1 AND key = $2 AND field = $3", s.edits)))
            .bind(t.name)
            .bind(key_text)
            .bind(field.name)
            .execute(&mut *conn)
            .await?;
        "unset"
    } else {
        sqlx::query(sql(format!(
            "INSERT INTO {}.fields (target, key, field, value, original, change_set) VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (target, key, field) DO UPDATE SET value = EXCLUDED.value, change_set = EXCLUDED.change_set",
            s.edits
        )))
        .bind(t.name)
        .bind(key_text)
        .bind(field.name)
        .bind(&new)
        .bind(&original)
        .bind(run.cs)
        .execute(&mut *conn)
        .await?;
        "set"
    };
    write_field(conn, s, t, key, field.name, &new).await?;
    log(conn, run, t.name, key_text, Some(field.name), action, Some(&current), Some(&new)).await?;
    run.effects.insert(field.effect);
    if t.name == "mob" {
        run.facts = load_facts(conn, s).await?;
    }
    Ok(())
}

/// `UPDATE` of one column of a row.
pub async fn write_field(conn: &mut PgConnection, s: &Schemas, t: &Target, key: &[i64], field: &str, value: &Value) -> Result<u64> {
    let f = user(t.field(field))?;
    let query = sql(format!("UPDATE {} SET {} = {} WHERE {}", s.table(t), f.name, f.sql.cast("$1"), t.where_sql(&s.world, 2)));
    let mut q = sqlx::query(query).bind(value);
    for k in key {
        q = q.bind(*k);
    }
    Ok(q.execute(&mut *conn).await?.rows_affected())
}

/// `INSERT` of a whole row: the key's columns and every editable field.
pub async fn insert_row(conn: &mut PgConnection, s: &Schemas, t: &Target, key: &[i64], row: &Map<String, Value>) -> Result<()> {
    let (key_cols, key_vals) = match t.name {
        "spawn_mob" => ("mob_area_id, seq".to_owned(), format!("(SELECT id FROM {}.mob_areas WHERE map = $1 AND seq = $2), $3", s.world)),
        _ => (t.key_cols.join(", "), (1..=key.len()).map(|i| format!("${i}")).collect::<Vec<_>>().join(", ")),
    };
    let names = t.fields.iter().map(|f| f.name).collect::<Vec<_>>().join(", ");
    let values = t.fields.iter().enumerate().map(|(i, f)| f.sql.cast(&format!("${}", key.len() + 1 + i))).collect::<Vec<_>>().join(", ");
    let mut q = sqlx::query(sql(format!("INSERT INTO {} ({key_cols}, {names}) VALUES ({key_vals}, {values})", s.table(t))));
    for k in key {
        q = q.bind(*k);
    }
    for f in t.fields {
        q = q.bind(row.get(f.name).cloned().unwrap_or(Value::Null));
    }
    q.execute(&mut *conn).await?;
    Ok(())
}

pub async fn delete_row(conn: &mut PgConnection, s: &Schemas, t: &Target, key: &[i64]) -> Result<u64> {
    let mut q = sqlx::query(sql(format!("DELETE FROM {} WHERE {}", s.table(t), t.where_sql(&s.world, 1))));
    for k in key {
        q = q.bind(*k);
    }
    Ok(q.execute(&mut *conn).await?.rows_affected())
}

async fn add_row(conn: &mut PgConnection, run: &mut Run<'_>, t: &Target, key: &[i64], key_text: &str, fields: &Value) -> Result<()> {
    if !t.can_add {
        return refuse("rows of this kind cannot be added");
    }
    let s = run.s;
    let mut parent_added = false;
    match t.name {
        "spawn_mob" => {
            let area = user(registry::target("spawn_area"))?;
            if read_row(conn, s, area, &key[..2]).await?.is_none() {
                return refuse(format!("its area {} does not exist", format_key(&key[..2])));
            }
            parent_added = row_action(conn, s, "spawn_area", &format_key(&key[..2])).await?.as_deref() == Some("add");
        }
        "obelisk_guard" | "boss_follower" => {
            let parent = user(registry::target(if t.name == "obelisk_guard" { "obelisk" } else { "boss_group" }))?;
            if read_row(conn, s, parent, &key[..1]).await?.is_none() {
                return refuse(format!("its {} {} does not exist", parent.name, key[0]));
            }
        }
        _ => {}
    }
    user(t.check_new_key(key, parent_added))?;
    if read_row(conn, s, t, key).await?.is_some() {
        return refuse("a row with this key exists already");
    }
    if row_action(conn, s, t.name, key_text).await?.as_deref() == Some("remove") {
        return refuse("a row with this key was removed by an earlier change: undo that change instead");
    }
    let none = Map::new();
    let map = row_map(conn, s, t, key, &none).await?;
    let ctx = Ctx { facts: &run.facts, map, current: &none, key };
    let row = user(t.check_new_row(fields, &ctx))?;
    insert_row(conn, s, t, key, &row).await?;
    record_added(conn, run, t.name, key_text, &row).await?;
    log(conn, run, t.name, key_text, None, "add", None, Some(&Value::Object(row))).await?;
    run.effects.insert(Effect::Restart);
    Ok(())
}

/// The edits' own record of an added row: a `rows` line and the fields (without originals).
async fn record_added(conn: &mut PgConnection, run: &Run<'_>, target: &str, key: &str, row: &Map<String, Value>) -> Result<()> {
    let s = run.s;
    sqlx::query(sql(format!("INSERT INTO {}.rows (target, key, action, original, change_set) VALUES ($1, $2, 'add', NULL, $3)", s.edits)))
        .bind(target)
        .bind(key)
        .bind(run.cs)
        .execute(&mut *conn)
        .await?;
    for (field, value) in row {
        sqlx::query(sql(format!(
            "INSERT INTO {}.fields (target, key, field, value, original, change_set) VALUES ($1, $2, $3, $4, NULL, $5)",
            s.edits
        )))
        .bind(target)
        .bind(key)
        .bind(field)
        .bind(value)
        .bind(run.cs)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// One row as a snapshot node (no children).
async fn node(conn: &mut PgConnection, s: &Schemas, t: &Target, key: &[i64]) -> Result<Snapshot> {
    let Some(row) = read_row(conn, s, t, key).await? else { return refuse(format!("{} {} does not exist", t.name, format_key(key))) };
    let key_text = format_key(key);
    let fields = t.fields.iter().map(|f| (f.name.to_owned(), canon(row.get(f.name).unwrap_or(&Value::Null)))).collect();
    let added = row_action(conn, s, t.name, &key_text).await?.as_deref() == Some("add");
    let edits: Vec<(String, Value, Option<Value>)> =
        sqlx::query_as(sql(format!("SELECT field, value, original FROM {}.fields WHERE target = $1 AND key = $2 ORDER BY field", s.edits)))
            .bind(t.name)
            .bind(&key_text)
            .fetch_all(&mut *conn)
            .await?;
    Ok(Snapshot {
        target: t.name.to_owned(),
        key: key_text,
        added,
        row: fields,
        edits: edits.into_iter().map(|(field, value, original)| FieldEdit { field, value, original }).collect(),
        children: Vec::new(),
    })
}

/// A row with its child rows (the monsters of a spawn area).
pub async fn snapshot(conn: &mut PgConnection, s: &Schemas, t: &Target, key: &[i64]) -> Result<Snapshot> {
    let mut snap = node(conn, s, t, key).await?;
    for child in registry::children_of(t.name) {
        let ct = user(registry::target(child))?;
        let seqs: Vec<i64> = sqlx::query_scalar(sql(format!(
            "SELECT seq::bigint FROM {}.mob_area_mobs WHERE mob_area_id = (SELECT id FROM {}.mob_areas WHERE map = $1 AND seq = $2) ORDER BY seq",
            s.world, s.world
        )))
        .bind(key[0])
        .bind(key[1])
        .fetch_all(&mut *conn)
        .await?;
        for seq in seqs {
            snap.children.push(node(conn, s, ct, &[key[0], key[1], seq]).await?);
        }
    }
    Ok(snap)
}

async fn remove_row(conn: &mut PgConnection, run: &mut Run<'_>, t: &Target, key: &[i64], key_text: &str) -> Result<()> {
    if !t.can_remove {
        return refuse("rows of this kind cannot be removed");
    }
    let s = run.s;
    let snap = snapshot(conn, s, t, key).await?;
    for n in snap.nodes() {
        sqlx::query(sql(format!("DELETE FROM {}.fields WHERE target = $1 AND key = $2", s.edits)))
            .bind(&n.target)
            .bind(&n.key)
            .execute(&mut *conn)
            .await?;
        sqlx::query(sql(format!("DELETE FROM {}.rows WHERE target = $1 AND key = $2 AND action = 'add'", s.edits)))
            .bind(&n.target)
            .bind(&n.key)
            .execute(&mut *conn)
            .await?;
    }
    let json = serde_json::to_value(&snap)?;
    if !snap.added {
        sqlx::query(sql(format!(
            "INSERT INTO {}.rows (target, key, action, original, change_set) VALUES ($1, $2, 'remove', $3, $4)",
            s.edits
        )))
        .bind(t.name)
        .bind(key_text)
        .bind(&json)
        .bind(run.cs)
        .execute(&mut *conn)
        .await?;
    }
    delete_row(conn, s, t, key).await?;
    log(conn, run, t.name, key_text, None, "remove", Some(&json), None).await?;
    run.effects.insert(Effect::Restart);
    Ok(())
}

async fn restore_row(conn: &mut PgConnection, run: &mut Run<'_>, snapshot: &Value) -> Result<()> {
    let snap: Snapshot = serde_json::from_value(snapshot.clone()).context("the snapshot to restore")?;
    let s = run.s;
    for n in snap.nodes() {
        let t = user(registry::target(&n.target))?;
        let key = user(t.parse_key(&n.key))?;
        if read_row(conn, s, t, &key).await?.is_some() {
            return refuse(format!("{} {} exists already", n.target, n.key));
        }
        insert_row(conn, s, t, &key, &n.row).await?;
        if n.added {
            record_added(conn, run, &n.target, &n.key, &n.row).await?;
        } else {
            sqlx::query(sql(format!("DELETE FROM {}.rows WHERE target = $1 AND key = $2 AND action = 'remove'", s.edits)))
                .bind(&n.target)
                .bind(&n.key)
                .execute(&mut *conn)
                .await?;
            for e in &n.edits {
                sqlx::query(sql(format!(
                    "INSERT INTO {}.fields (target, key, field, value, original, change_set) VALUES ($1, $2, $3, $4, $5, $6)",
                    s.edits
                )))
                .bind(&n.target)
                .bind(&n.key)
                .bind(&e.field)
                .bind(&e.value)
                .bind(&e.original)
                .bind(run.cs)
                .execute(&mut *conn)
                .await?;
            }
        }
    }
    let t = user(registry::target(&snap.target))?;
    let _ = t;
    log(conn, run, &snap.target, &snap.key, None, "restore", None, Some(snapshot)).await?;
    run.effects.insert(Effect::Restart);
    Ok(())
}

/// The draft columns as read: seq, target, key, field, action, value, made by, made at.
type DraftCols = (i32, String, String, Option<String>, String, Option<Value>, String, String);
/// The log columns an undo reads: target, key, field, action, before, after.
type UndoCols = (String, String, Option<String>, String, Option<Value>, Option<Value>);
/// The change-set columns `history` reads: id, made at, made by, message, undoes, undone by, the count of logged changes.
type HistoryCols = (i32, String, String, String, Option<i32>, Option<i32>, i64);
/// The log columns `history_detail` reads: seq, target, key, field, action, before, after.
type LogCols = (i32, String, String, Option<String>, String, Option<Value>, Option<Value>);

/// A line of the draft.
#[derive(Clone, Debug, Serialize)]
pub struct DraftLine {
    pub seq: i32,
    pub target: String,
    pub key: String,
    pub field: Option<String>,
    pub action: String,
    pub value: Option<Value>,
    pub made_by: String,
    pub made_at: String,
}

impl DraftLine {
    pub fn line(&self) -> Result<Line> {
        let need = |what: &str, v: Option<&Value>| {
            v.cloned().ok_or_else(|| anyhow::Error::new(Refused(format!("draft line {} ({what}) has no value", self.seq))))
        };
        let field = || self.field.clone().ok_or_else(|| anyhow::Error::new(Refused(format!("draft line {} has no field", self.seq))));
        let op = match self.action.as_str() {
            "set" => Op::Set { field: field()?, value: need("set", self.value.as_ref())? },
            "unset" => Op::Unset { field: field()? },
            "add" => Op::Add { fields: need("add", self.value.as_ref())? },
            "remove" => Op::Remove,
            other => return refuse(format!("unknown draft action {other:?}")),
        };
        Ok(Line { target: self.target.clone(), key: self.key.clone(), op })
    }
}

/// A change set in the history.
#[derive(Clone, Debug, Serialize)]
pub struct HistoryRow {
    pub id: i32,
    pub made_at: String,
    pub made_by: String,
    pub message: String,
    pub undoes: Option<i32>,
    pub undone_by: Option<i32>,
    pub changes: i64,
}

/// A logged change.
#[derive(Clone, Debug, Serialize)]
pub struct LogRow {
    pub seq: i32,
    pub target: String,
    pub key: String,
    pub field: Option<String>,
    pub action: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Status {
    pub defs_marker: Option<String>,
    pub world_marker: Option<String>,
    pub change_sets: i64,
    pub field_edits: i64,
    pub added_rows: i64,
    pub removed_rows: i64,
    pub draft_lines: i64,
}

/// The store: a pool and the schema names.
#[derive(Clone)]
pub struct Editor {
    pub pool: PgPool,
    pub s: Schemas,
}

impl Editor {
    pub fn new(pool: PgPool, s: Schemas) -> Self {
        Self { pool, s }
    }

    /// A transaction that holds the lock that keeps two Applies apart.
    pub async fn begin_locked(&self) -> Result<sqlx::Transaction<'static, sqlx::Postgres>> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(sql(format!("LOCK TABLE {}.change_sets IN EXCLUSIVE MODE", self.s.edits))).execute(&mut *tx).await?;
        Ok(tx)
    }

    pub async fn draft_list(&self) -> Result<Vec<DraftLine>> {
        let rows: Vec<DraftCols> = sqlx::query_as(sql(format!(
            "SELECT seq, target, key, field, action, value, made_by, made_at::text FROM {}.draft ORDER BY seq",
            self.s.edits
        )))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(seq, target, key, field, action, value, made_by, made_at)| DraftLine {
                seq,
                target,
                key,
                field,
                action,
                value,
                made_by,
                made_at,
            })
            .collect())
    }

    /// Checks a line against the tables and the draft so far (a dry run of Apply that is rolled back), then adds it. Returns its number.
    pub async fn draft_add(&self, line: &Line, by: &str) -> Result<i32> {
        let t = user(registry::target(&line.target))?;
        let key = user(t.parse_key(&line.key))?;
        let key_text = format_key(&key);
        let existing = self.draft_list().await?;
        if matches!(line.op, Op::Remove) {
            let pending = existing.iter().find(|d| d.target == t.name && d.key == key_text).or_else(|| {
                registry::children_of(t.name)
                    .iter()
                    .find_map(|c| existing.iter().find(|d| d.target == *c && d.key.starts_with(&format!("{key_text}/"))))
            });
            if let Some(p) = pending {
                return refuse(format!("{} {key_text} has a pending change in the draft (line {}): remove that line first", t.name, p.seq));
            }
        }
        self.dry_run(&existing, line).await?;
        let (field, action, value): (Option<&str>, &str, Option<&Value>) = match &line.op {
            Op::Set { field, value } => (Some(field), "set", Some(value)),
            Op::Unset { field } => (Some(field), "unset", None),
            Op::Add { fields } => (None, "add", Some(fields)),
            Op::Remove => (None, "remove", None),
            Op::Restore { .. } => return refuse("a restore cannot be drafted"),
        };
        let seq = sqlx::query_scalar(sql(format!(
            "INSERT INTO {}.draft (target, key, field, action, value, made_by) VALUES ($1, $2, $3, $4, $5, $6) RETURNING seq",
            self.s.edits
        )))
        .bind(t.name)
        .bind(&key_text)
        .bind(field)
        .bind(action)
        .bind(value)
        .bind(by)
        .fetch_one(&self.pool)
        .await?;
        Ok(seq)
    }

    /// The draft plus one more line applied inside a transaction that is rolled back. It holds the Apply lock and puts the change set
    /// numbering back afterwards, so that dry runs leave no gaps in the history.
    async fn dry_run(&self, draft: &[DraftLine], extra: &Line) -> Result<()> {
        let mut tx = self.begin_locked().await?;
        let sequence: String = sqlx::query_scalar(sql(format!("SELECT pg_get_serial_sequence('{}.change_sets', 'id')", self.s.edits)))
            .fetch_one(&mut *tx)
            .await?;
        let (last, called): (i64, bool) =
            sqlx::query_as(sql(format!("SELECT last_value, is_called FROM {sequence}"))).fetch_one(&mut *tx).await?;
        let cs = new_change_set(&mut tx, &self.s, "dry run", "dry run", None).await?;
        let mut run = Run::new(&mut tx, &self.s, cs).await?;
        for d in draft {
            apply_line(&mut tx, &mut run, &d.line()?).await?;
        }
        let result = apply_line(&mut tx, &mut run, extra).await;
        sqlx::query(sql(format!("SELECT setval('{sequence}', {last}, {called})"))).execute(&mut *tx).await?;
        tx.rollback().await?;
        result
    }

    pub async fn draft_remove(&self, seq: i32) -> Result<bool> {
        Ok(sqlx::query(sql(format!("DELETE FROM {}.draft WHERE seq = $1", self.s.edits)))
            .bind(seq)
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0)
    }

    pub async fn draft_clear(&self) -> Result<u64> {
        Ok(sqlx::query(sql(format!("DELETE FROM {}.draft", self.s.edits))).execute(&self.pool).await?.rows_affected())
    }

    /// Apply (section 4): the whole draft as one change set.
    pub async fn apply_draft(&self, message: &str, by: &str) -> Result<Applied> {
        if message.trim().is_empty() {
            return refuse("an Apply needs a message");
        }
        let mut tx = self.begin_locked().await?;
        let rows: Vec<DraftCols> = sqlx::query_as(sql(format!(
            "SELECT seq, target, key, field, action, value, made_by, made_at::text FROM {}.draft ORDER BY seq",
            self.s.edits
        )))
        .fetch_all(&mut *tx)
        .await?;
        if rows.is_empty() {
            return refuse("the draft is empty");
        }
        let problems = ensure_applied(&mut tx, &self.s).await?;
        let cs = new_change_set(&mut tx, &self.s, by, message, None).await?;
        let mut run = Run::new(&mut tx, &self.s, cs).await?;
        for (seq, target, key, field, action, value, made_by, made_at) in rows {
            let d = DraftLine { seq, target, key, field, action, value, made_by, made_at };
            apply_line(&mut tx, &mut run, &d.line()?).await.map_err(|e| match e.downcast::<Refused>() {
                Ok(Refused(m)) => anyhow::Error::new(Refused(format!("draft line {seq}: {m}"))),
                Err(e) => e.context(format!("draft line {seq}")),
            })?;
        }
        if run.seq == 0 {
            return refuse("the draft changes nothing (every value is the one in force)");
        }
        sqlx::query(sql(format!("DELETE FROM {}.draft", self.s.edits))).execute(&mut *tx).await?;
        let applied = Applied { change_set: cs, changes: run.seq as usize, effects: run.effects, problems };
        tx.commit().await?;
        Ok(applied)
    }

    /// Undo (section 4): a new change set that reverses the log of `id`.
    pub async fn undo(&self, id: i32, by: &str) -> Result<Applied> {
        let mut tx = self.begin_locked().await?;
        let problems = ensure_applied(&mut tx, &self.s).await?;
        let found: Option<(String, Option<i32>, Option<i32>)> =
            sqlx::query_as(sql(format!("SELECT message, undoes, undone_by FROM {}.change_sets WHERE id = $1", self.s.edits)))
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some((message, undoes, undone_by)) = found else { return refuse(format!("there is no change set #{id}")) };
        if let Some(by) = undone_by {
            return refuse(format!("change set #{id} is undone already (by #{by})"));
        }
        let e = &self.s.edits;
        let conflicts: Vec<(i32, String, String, Option<String>)> = sqlx::query_as(sql(format!(
            "SELECT DISTINCT l.change_set, l.target, l.key, l.field FROM {e}.log l JOIN {e}.change_sets c ON c.id = l.change_set
             WHERE c.id > $1 AND c.undone_by IS NULL AND NOT (c.undoes IS NOT NULL AND c.undoes > $1)
               AND EXISTS (SELECT 1 FROM {e}.log m WHERE m.change_set = $1 AND m.target = l.target AND m.key = l.key
                           AND (m.field IS NULL OR l.field IS NULL OR m.field = l.field))
             ORDER BY 1, 2, 3, 4"
        )))
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        if !conflicts.is_empty() {
            let list: Vec<String> = conflicts
                .iter()
                .map(|(cs, t, k, f)| format!("{t} {k}{} (change set #{cs})", f.as_ref().map(|f| format!(" {f}")).unwrap_or_default()))
                .collect();
            return refuse(format!("change set #{id} cannot be undone: a later change touched {}", list.join(", ")));
        }
        let logged: Vec<UndoCols> = sqlx::query_as(sql(format!(
            "SELECT target, key, field, action, before, after FROM {e}.log WHERE change_set = $1 ORDER BY seq DESC"
        )))
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        let cs = new_change_set(&mut tx, &self.s, by, &format!("Undo #{id}: {message}"), Some(id)).await?;
        let mut run = Run::new(&mut tx, &self.s, cs).await?;
        for (target, key, field, action, before, after) in logged {
            let op = match (action.as_str(), field) {
                ("set" | "unset", Some(field)) => Op::Set { field, value: before.clone().unwrap_or(Value::Null) },
                ("add", _) => Op::Remove,
                ("remove", _) => Op::Restore { snapshot: before.clone().unwrap_or(Value::Null) },
                ("restore", _) => {
                    let _ = after;
                    Op::Remove
                }
                (other, _) => return refuse(format!("the log has a line {other:?} that cannot be reversed")),
            };
            apply_line(&mut tx, &mut run, &Line { target, key, op }).await?;
        }
        sqlx::query(sql(format!("UPDATE {e}.change_sets SET undone_by = $2 WHERE id = $1"))).bind(id).bind(cs).execute(&mut *tx).await?;
        if let Some(earlier) = undoes {
            sqlx::query(sql(format!("UPDATE {e}.change_sets SET undone_by = NULL WHERE id = $1"))).bind(earlier).execute(&mut *tx).await?;
        }
        let applied = Applied { change_set: cs, changes: run.seq as usize, effects: run.effects, problems };
        tx.commit().await?;
        Ok(applied)
    }

    pub async fn history(&self, limit: i64) -> Result<Vec<HistoryRow>> {
        let e = &self.s.edits;
        let rows: Vec<HistoryCols> = sqlx::query_as(sql(format!(
            "SELECT c.id, c.made_at::text, c.made_by, c.message, c.undoes, c.undone_by, (SELECT count(*) FROM {e}.log l WHERE l.change_set = c.id)
             FROM {e}.change_sets c ORDER BY c.id DESC LIMIT $1"
        )))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, made_at, made_by, message, undoes, undone_by, changes)| HistoryRow {
                id,
                made_at,
                made_by,
                message,
                undoes,
                undone_by,
                changes,
            })
            .collect())
    }

    /// The logged changes of one change set.
    pub async fn history_detail(&self, id: i32) -> Result<Vec<LogRow>> {
        let rows: Vec<LogCols> = sqlx::query_as(sql(format!(
            "SELECT seq, target, key, field, action, before, after FROM {}.log WHERE change_set = $1 ORDER BY seq",
            self.s.edits
        )))
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(seq, target, key, field, action, before, after)| LogRow { seq, target, key, field, action, before, after })
            .collect())
    }

    pub async fn status(&self) -> Result<Status> {
        let e = &self.s.edits;
        let count = |table: &str, filter: &str| sql(format!("SELECT count(*) FROM {e}.{table} {filter}"));
        let marker = |schema: String| async move {
            let exists: bool =
                sqlx::query_scalar(sql(format!("SELECT to_regclass('{schema}.edits_applied') IS NOT NULL"))).fetch_one(&self.pool).await?;
            if !exists {
                return Ok::<_, anyhow::Error>(None);
            }
            Ok(sqlx::query_scalar::<_, String>(sql(format!("SELECT applied_at::text FROM {schema}.edits_applied LIMIT 1")))
                .fetch_optional(&self.pool)
                .await?)
        };
        Ok(Status {
            defs_marker: marker(self.s.defs.clone()).await?,
            world_marker: marker(self.s.world.clone()).await?,
            change_sets: sqlx::query_scalar(count("change_sets", "")).fetch_one(&self.pool).await?,
            field_edits: sqlx::query_scalar(count("fields", "")).fetch_one(&self.pool).await?,
            added_rows: sqlx::query_scalar(count("rows", "WHERE action = 'add'")).fetch_one(&self.pool).await?,
            removed_rows: sqlx::query_scalar(count("rows", "WHERE action = 'remove'")).fetch_one(&self.pool).await?,
            draft_lines: sqlx::query_scalar(count("draft", "")).fetch_one(&self.pool).await?,
        })
    }
}

pub async fn new_change_set(conn: &mut PgConnection, s: &Schemas, by: &str, message: &str, undoes: Option<i32>) -> Result<i32> {
    Ok(sqlx::query_scalar(sql(format!("INSERT INTO {}.change_sets (made_by, message, undoes) VALUES ($1, $2, $3) RETURNING id", s.edits)))
        .bind(by)
        .bind(message)
        .bind(undoes)
        .fetch_one(&mut *conn)
        .await?)
}
