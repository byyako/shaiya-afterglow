//! `data-editor apply` (`docs/plans/done/data-editing.md` section 3): writes every edit back into `defs` and `world`, which a fresh load of the
//! original files replaces. A one-row marker table in each loaded schema (`edits_applied`) tells a fresh schema from one the edits are
//! in already: a fresh schema first gets the edits' originals refreshed from its tables and every disagreement reported.

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::{AssertSqlSafe, PgConnection};

use crate::registry::{self, Schemas, Side, Target, canon, format_key};
use crate::store::{Editor, Snapshot, delete_row, insert_row, read_row, user, write_field};

fn sql(text: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(text)
}

/// Something an apply found that the operator should look at; the edit is left alone.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Problem {
    pub change_set: Option<i32>,
    pub target: String,
    pub key: String,
    pub field: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    /// The schemas that were fresh (no marker): `defs`, `world`.
    pub fresh: Vec<String>,
    /// Schemas that do not exist (nothing to write into).
    pub missing: Vec<String>,
    /// Table values written (edited fields that differed, rows added, rows removed).
    pub written: usize,
    pub problems: Vec<Problem>,
}

impl Report {
    pub(crate) fn problem(&mut self, change_set: Option<i32>, target: &str, key: &str, field: Option<&str>, text: impl Into<String>) {
        self.problems.push(Problem {
            change_set,
            target: target.to_owned(),
            key: key.to_owned(),
            field: field.map(str::to_owned),
            text: text.into(),
        });
    }
}

type FieldRow = (String, String, String, Value, Option<Value>, i32);

/// Writes all edits. The caller owns the transaction (and rolls it back for a dry run).
pub async fn apply_all(conn: &mut PgConnection, s: &Schemas) -> Result<Report> {
    let mut report = Report::default();
    for side in [Side::Defs, Side::World] {
        apply_side(conn, s, side, &mut report).await?;
    }
    // The settings' overrides live in `content`, which no load replaces: no marker, always written.
    crate::settings::apply_edits(conn, s, &mut report).await?;
    crate::rules::apply_edits(conn, s, &mut report).await?;
    Ok(report)
}

/// Before a new Apply or Undo: a loaded schema without its marker (a fresh load) gets the edits written first, so that the new change
/// meets tables that hold the edits in force. Returns what that found.
pub async fn ensure_applied(conn: &mut PgConnection, s: &Schemas) -> Result<Vec<Problem>> {
    for schema in [&s.defs, &s.world] {
        let exists: bool = sqlx::query_scalar("SELECT to_regnamespace($1::text) IS NOT NULL").bind(schema).fetch_one(&mut *conn).await?;
        let marker: bool =
            sqlx::query_scalar(sql(format!("SELECT to_regclass('{schema}.edits_applied') IS NOT NULL"))).fetch_one(&mut *conn).await?;
        if exists && !marker {
            return Ok(apply_all(conn, s).await?.problems);
        }
    }
    Ok(Vec::new())
}

pub(crate) async fn apply_side(conn: &mut PgConnection, s: &Schemas, side: Side, report: &mut Report) -> Result<()> {
    let schema = s.of(side).to_owned();
    let exists: bool = sqlx::query_scalar("SELECT to_regnamespace($1::text) IS NOT NULL").bind(&schema).fetch_one(&mut *conn).await?;
    if !exists {
        report.missing.push(schema);
        return Ok(());
    }
    let marker: bool =
        sqlx::query_scalar(sql(format!("SELECT to_regclass('{schema}.edits_applied') IS NOT NULL"))).fetch_one(&mut *conn).await?;
    let targets: Vec<&Target> = registry::all().iter().filter(|t| t.side == side).collect();
    let names: Vec<String> = targets.iter().map(|t| t.name.to_owned()).collect();

    let fields: Vec<FieldRow> = sqlx::query_as(sql(format!(
        "SELECT target, key, field, value, original, change_set FROM {}.fields WHERE target = ANY($1) ORDER BY target, key, field",
        s.edits
    )))
    .bind(&names)
    .fetch_all(&mut *conn)
    .await?;
    let rows: Vec<(String, String, String, Option<Value>, i32)> = sqlx::query_as(sql(format!(
        "SELECT target, key, action, original, change_set FROM {}.rows WHERE target = ANY($1) ORDER BY target, key",
        s.edits
    )))
    .bind(&names)
    .fetch_all(&mut *conn)
    .await?;

    // Edits that cannot be written are left alone and never touched again below. Added rows that met an imported row at the last load
    // stay skipped (the marker keeps their list), so a later apply does not mistake the imported row for its own.
    let mut skip: Vec<(String, String)> = Vec::new();
    let mut collisions: Vec<String> = Vec::new();
    if marker {
        let kept: Option<Vec<String>> =
            sqlx::query_scalar(sql(format!("SELECT skipped FROM {schema}.edits_applied LIMIT 1"))).fetch_optional(&mut *conn).await?;
        for entry in kept.unwrap_or_default() {
            let Some((target, key)) = entry.split_once('\t') else { continue };
            let Some((_, _, _, _, cs)) = rows.iter().find(|r| r.0 == target && r.1 == key && r.2 == "add") else { continue };
            report.problem(
                Some(*cs),
                target,
                key,
                None,
                "the import had a row with this key at the last load; the row added by this change is not written",
            );
            skip.push((target.to_owned(), key.to_owned()));
            collisions.push(entry);
        }
    } else {
        report.fresh.push(schema.clone());
        refresh(conn, s, &fields, &rows, report, &mut skip).await?;
    }

    // Rows removed: children first, so that a parent's removal does not make them look gone.
    for t in targets.iter().rev() {
        for (_, key_text, _, _, cs) in rows.iter().filter(|r| r.0 == t.name && r.2 == "remove") {
            let key = user(t.parse_key(key_text))?;
            if skip.iter().any(|(tn, k)| tn == t.name && k == key_text) {
                continue;
            }
            if delete_row(conn, s, t, &key).await? > 0 {
                report.written += 1;
            } else if !marker {
                report.problem(
                    Some(*cs),
                    t.name,
                    key_text,
                    None,
                    "the row removed by this change is gone already (the files changed under the edit)",
                );
            }
        }
    }

    // Rows added: parents before children.
    for t in &targets {
        for (_, key_text, _, _, cs) in rows.iter().filter(|r| r.0 == t.name && r.2 == "add") {
            let key = user(t.parse_key(key_text))?;
            if skip.iter().any(|(tn, k)| tn == t.name && k == key_text) {
                continue;
            }
            if t.name == "spawn_mob" {
                let area = format_key(&key[..2]);
                if skip.iter().any(|(tn, k)| tn == "spawn_area" && *k == area) {
                    report.problem(
                        Some(*cs),
                        t.name,
                        key_text,
                        None,
                        format!("its area {area} was not added; the monster was not written"),
                    );
                    skip.push((t.name.to_owned(), key_text.clone()));
                    continue;
                }
            }
            let present = read_row(conn, s, t, &key).await?.is_some();
            if present && !marker {
                report.problem(
                    Some(*cs),
                    t.name,
                    key_text,
                    None,
                    "the import has a row with this key now; the row added by this change was not written",
                );
                skip.push((t.name.to_owned(), key_text.clone()));
                collisions.push(format!("{}\t{key_text}", t.name));
                continue;
            }
            if present {
                continue;
            }
            let mut row = Map::new();
            for (_, _, field, value, _, _) in fields.iter().filter(|f| f.0 == t.name && f.1 == *key_text) {
                row.insert(field.clone(), value.clone());
            }
            if t.fields.iter().any(|f| !row.contains_key(f.name)) {
                report.problem(Some(*cs), t.name, key_text, None, "the added row's fields are incomplete; it was not written");
                skip.push((t.name.to_owned(), key_text.clone()));
                continue;
            }
            insert_row(conn, s, t, &key, &row)
                .await
                .with_context(|| format!("writing the added {} {key_text} (change set #{cs})", t.name))?;
            report.written += 1;
        }
    }

    // Edited fields.
    for (target, key_text, field, value, _, cs) in &fields {
        if skip.iter().any(|(tn, k)| tn == target && k == key_text) {
            continue;
        }
        let t = user(registry::target(target))?;
        let key = user(t.parse_key(key_text))?;
        let Some(row) = read_row(conn, s, t, &key).await? else {
            report.problem(Some(*cs), target, key_text, Some(field), "the row does not exist any more; the edit was left alone");
            continue;
        };
        if row.get(field).map(canon).as_ref() == Some(&canon(value)) {
            continue;
        }
        write_field(conn, s, t, &key, field, value).await?;
        report.written += 1;
    }

    sqlx::query(sql(format!("CREATE TABLE IF NOT EXISTS {schema}.edits_applied (applied_at timestamptz NOT NULL DEFAULT now(), skipped text[] NOT NULL DEFAULT '{{}}')"))).execute(&mut *conn).await?;
    sqlx::query(sql(format!("DELETE FROM {schema}.edits_applied"))).execute(&mut *conn).await?;
    sqlx::query(sql(format!("INSERT INTO {schema}.edits_applied (skipped) VALUES ($1)"))).bind(&collisions).execute(&mut *conn).await?;
    Ok(())
}

/// A fresh schema: the originals of the edits become what the tables hold now, and the differences are reported.
async fn refresh(
    conn: &mut PgConnection,
    s: &Schemas,
    fields: &[FieldRow],
    rows: &[(String, String, String, Option<Value>, i32)],
    report: &mut Report,
    skip: &mut Vec<(String, String)>,
) -> Result<()> {
    for (target, key_text, field, value, original, cs) in fields {
        let Some(original) = original else { continue };
        let t = user(registry::target(target))?;
        let key = user(t.parse_key(key_text))?;
        match read_row(conn, s, t, &key).await? {
            None => {
                report.problem(
                    Some(*cs),
                    target,
                    key_text,
                    Some(field),
                    "the row does not exist in the new files; the edit was left alone",
                );
                skip.push((target.clone(), key_text.clone()));
            }
            Some(row) => {
                let now = canon(row.get(field).unwrap_or(&Value::Null));
                if now != canon(original) {
                    report.problem(Some(*cs), target, key_text, Some(field), format!("the content under your edit changed {field} {original} -> {now} (your edit sets {value}); the edit stays, its original is now {now}"));
                    sqlx::query(sql(format!("UPDATE {}.fields SET original = $4 WHERE target = $1 AND key = $2 AND field = $3", s.edits)))
                        .bind(target)
                        .bind(key_text)
                        .bind(field)
                        .bind(&now)
                        .execute(&mut *conn)
                        .await?;
                }
            }
        }
    }
    for (target, key_text, action, original, cs) in rows {
        if action != "remove" {
            continue;
        }
        let t = user(registry::target(target))?;
        let key = user(t.parse_key(key_text))?;
        if read_row(conn, s, t, &key).await?.is_none() {
            report.problem(
                Some(*cs),
                target,
                key_text,
                None,
                "the row removed by this change is gone already (the files changed under the edit)",
            );
            skip.push((target.clone(), key_text.clone()));
            continue;
        }
        // Keep the snapshot Undo restores from in step with the new files.
        let Some(original) = original else { continue };
        let mut snap: Snapshot = serde_json::from_value(original.clone())?;
        refresh_node(conn, s, &mut snap).await?;
        for child in snap.children.iter_mut() {
            refresh_node(conn, s, child).await?;
        }
        sqlx::query(sql(format!("UPDATE {}.rows SET original = $3 WHERE target = $1 AND key = $2", s.edits)))
            .bind(target)
            .bind(key_text)
            .bind(serde_json::to_value(&snap)?)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// A snapshot node's row values from the table, when the row is still there.
async fn refresh_node(conn: &mut PgConnection, s: &Schemas, n: &mut Snapshot) -> Result<()> {
    let t = user(registry::target(&n.target))?;
    let key = user(t.parse_key(&n.key))?;
    if let Some(row) = read_row(conn, s, t, &key).await? {
        n.row = t.fields.iter().map(|f| (f.name.to_owned(), canon(row.get(f.name).unwrap_or(&Value::Null)))).collect();
    }
    Ok(())
}

impl Editor {
    /// `data-editor apply`: one transaction, under the same lock as Apply.
    pub async fn apply_edits(&self) -> Result<Report> {
        let mut tx = self.begin_locked().await?;
        let report = apply_all(&mut tx, &self.s).await?;
        tx.commit().await?;
        Ok(report)
    }

    /// The same, rolled back: what `apply` would write and report.
    pub async fn apply_dry_run(&self) -> Result<Report> {
        let mut tx = self.begin_locked().await?;
        let report = apply_all(&mut tx, &self.s).await?;
        tx.rollback().await?;
        Ok(report)
    }

    /// Puts every table back to the original files' values (each field to its original, added rows deleted, removed rows restored) and
    /// forgets the markers. Used by `import --replace` before the edits are emptied.
    pub async fn revert_all(&self, conn: &mut PgConnection) -> Result<()> {
        let s = &self.s;
        let fields: Vec<(String, String, String, Value)> = sqlx::query_as(sql(format!(
            "SELECT target, key, field, original FROM {}.fields WHERE original IS NOT NULL AND target NOT IN ('setting', 'rule') ORDER BY target, key, field",
            s.edits
        )))
        .fetch_all(&mut *conn)
        .await?;
        for (target, key_text, field, original) in fields {
            let t = user(registry::target(&target))?;
            let key = user(t.parse_key(&key_text))?;
            write_field(conn, s, t, &key, &field, &original).await?;
        }
        let added: Vec<(String, String)> =
            sqlx::query_as(sql(format!("SELECT target, key FROM {}.rows WHERE action = 'add'", s.edits))).fetch_all(&mut *conn).await?;
        for t in registry::all().iter().rev().filter(|t| t.side != Side::Content) {
            for (_, key_text) in added.iter().filter(|a| a.0 == t.name) {
                delete_row(conn, s, t, &user(t.parse_key(key_text))?).await?;
            }
        }
        let removed: Vec<Value> = sqlx::query_scalar(sql(format!(
            "SELECT original FROM {}.rows WHERE action = 'remove' AND original IS NOT NULL AND target NOT IN ('setting', 'rule') ORDER BY target, key",
            s.edits
        )))
        .fetch_all(&mut *conn)
        .await?;
        for original in removed {
            let snap: Snapshot = serde_json::from_value(original)?;
            for n in snap.nodes() {
                let t = user(registry::target(&n.target))?;
                let key = user(t.parse_key(&n.key))?;
                if read_row(conn, s, t, &key).await?.is_some() {
                    continue;
                }
                let mut row = n.row.clone();
                for e in &n.edits {
                    if let Some(o) = &e.original {
                        row.insert(e.field.clone(), o.clone());
                    }
                }
                insert_row(conn, s, t, &key, &row).await?;
            }
        }
        crate::settings::revert(conn, s).await?;
        crate::rules::revert(conn, s).await?;
        for schema in [&s.defs, &s.world] {
            sqlx::query(sql(format!("DROP TABLE IF EXISTS {schema}.edits_applied"))).execute(&mut *conn).await?;
        }
        Ok(())
    }
}

/// A key as the text the edits tables use (re-exported for the command line).
pub fn key_text(key: &[i64]) -> String {
    format_key(key)
}
