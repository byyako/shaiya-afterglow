//! The `rule` target (`docs/plans/ext-2-rules.md` section 4.5): the data editor's layer of the game server's rules. An override is a
//! row of `content.rule_values (name, rule)`, key = the rule's name, one field `rule` (a JSON object, the rule's keys as in the file).
//! Setting a rule that has no row adds one; removing the row falls back to the layers below (the extensions' rules, the settings'
//! rules). A table of `{"enabled": false}` alone turns off the rule of that name in a lower layer, which the editor cannot check (it
//! does not know the file): `game-server rules` reports an unmatched one, as the server's start does. The change goes through the edits
//! tables like every target (History, Undo, export and import work as usual) but the table lives in the `content` schema, which no
//! load replaces, so there is no marker: `apply` always writes it. Values are checked by the game server's own parser
//! (`game_server::rules::check_rule`); a change takes effect at the console's `reload`.
//!
//! The keys of the other targets are numbers; this one is a name, so [`apply_line`] and the apply and revert functions here keep
//! clear of the numeric key code in `store` and `apply` (as `crate::settings`).

use anyhow::Result;
use serde_json::{Map, Value, json};
use sqlx::{AssertSqlSafe, PgConnection};

use crate::apply::Report;
use crate::registry::{Check, Effect, Schemas};
use crate::store::{FieldEdit, Line, Op, Refused, Run, Snapshot, log, record_added, refuse, row_action, user};

/// The target's name in the edits tables.
pub const TARGET: &str = "rule";

/// The only field.
const FIELD: &str = "rule";

fn sql(text: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(text)
}

/// A rule's name as a key: `^[a-z0-9][a-z0-9-]{0,39}$`, or `<extension>/<name>` to replace or turn off an extension's rule; a name
/// starting with `setting.` belongs to the settings. `ext.<name>.<key>` (an extension's own options) is reserved for chunk 5.
pub fn key(text: &str) -> Check<String> {
    let name = text.trim();
    if name.starts_with("ext.") {
        return Err(format!("{name}: extension options are reserved for chunk 5 (docs/plans/ext-1-foundation.md 7.5)"));
    }
    game_server::rules::check_name(name)?;
    Ok(name.to_owned())
}

/// The shape of a rule's value: a JSON object. Whether the game server accepts it is checked with the key in hand ([`validated`]).
pub fn rule_object(value: &Value) -> Check<Value> {
    if value.is_object() { Ok(value.clone()) } else { Err("a rule is an object of its keys, e.g. {\"type\": \"level_gap\", \"gap\": 20}".into()) }
}

/// `value` when the game server accepts it as the rule `name`.
fn validated(name: &str, value: &Value) -> Check<Value> {
    let rule = rule_object(value)?;
    game_server::rules::check_rule(name, &rule).map_err(|e| format!("{e:#}"))?;
    Ok(rule)
}

/// The rule now in the table.
async fn current(conn: &mut PgConnection, s: &Schemas, name: &str) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar(sql(format!("SELECT rule FROM {}.rule_values WHERE name = $1", s.content)))
        .bind(name)
        .fetch_optional(&mut *conn)
        .await?)
}

/// Carries out one change of a rule (inside the caller's transaction) and logs it.
pub async fn apply_line(conn: &mut PgConnection, run: &mut Run<'_>, line: &Line) -> Result<()> {
    let name = user(key(&line.key))?;
    let done = match &line.op {
        Op::Set { field, value } => set(conn, run, &name, field, value).await,
        Op::Unset { field } => unset(conn, run, &name, field).await,
        Op::Add { fields } => add(conn, run, &name, fields).await,
        Op::Remove => remove(conn, run, &name).await,
        Op::Restore { snapshot } => restore(conn, run, snapshot).await,
    };
    done.map_err(|e| match e.downcast::<Refused>() {
        Ok(Refused(m)) => anyhow::Error::new(Refused(format!("rule {name}: {m}"))),
        Err(e) => e.context(format!("rule {name}")),
    })
}

fn only_field(field: &str) -> Result<()> {
    if field == FIELD { Ok(()) } else { refuse(format!("a rule has one field, {FIELD} (got {field:?})")) }
}

async fn set(conn: &mut PgConnection, run: &mut Run<'_>, name: &str, field: &str, value: &Value) -> Result<()> {
    only_field(field)?;
    let new = user(validated(name, value))?;
    let s = run.s;
    let Some(present) = current(conn, s, name).await? else {
        // No rule yet: setting one makes it.
        return add(conn, run, name, &json!({ FIELD: new })).await;
    };
    if present == new {
        return Ok(());
    }
    let added = row_action(conn, s, TARGET, name).await?.as_deref() == Some("add");
    let stored: Option<Option<Value>> =
        sqlx::query_scalar(sql(format!("SELECT original FROM {}.fields WHERE target = $1 AND key = $2 AND field = $3", s.edits)))
            .bind(TARGET)
            .bind(name)
            .bind(FIELD)
            .fetch_optional(&mut *conn)
            .await?;
    let before = present.clone();
    let after = new.clone();
    let original = match (stored, added) {
        (Some(original), _) => original,
        (None, true) => None,
        (None, false) => Some(before.clone()),
    };
    let action = if original.as_ref() == Some(&after) {
        sqlx::query(sql(format!("DELETE FROM {}.fields WHERE target = $1 AND key = $2 AND field = $3", s.edits)))
            .bind(TARGET)
            .bind(name)
            .bind(FIELD)
            .execute(&mut *conn)
            .await?;
        "unset"
    } else {
        sqlx::query(sql(format!(
            "INSERT INTO {}.fields (target, key, field, value, original, change_set) VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (target, key, field) DO UPDATE SET value = EXCLUDED.value, change_set = EXCLUDED.change_set",
            s.edits
        )))
        .bind(TARGET)
        .bind(name)
        .bind(FIELD)
        .bind(&after)
        .bind(&original)
        .bind(run.cs)
        .execute(&mut *conn)
        .await?;
        "set"
    };
    sqlx::query(sql(format!("UPDATE {}.rule_values SET rule = $2 WHERE name = $1", s.content)))
        .bind(name)
        .bind(&new)
        .execute(&mut *conn)
        .await?;
    log(conn, run, TARGET, name, Some(FIELD), action, Some(&before), Some(&after)).await?;
    run.effects.insert(Effect::Live);
    Ok(())
}

async fn unset(conn: &mut PgConnection, run: &mut Run<'_>, name: &str, field: &str) -> Result<()> {
    only_field(field)?;
    let stored: Option<Option<Value>> =
        sqlx::query_scalar(sql(format!("SELECT original FROM {}.fields WHERE target = $1 AND key = $2 AND field = $3", run.s.edits)))
            .bind(TARGET)
            .bind(name)
            .bind(FIELD)
            .fetch_optional(&mut *conn)
            .await?;
    match stored {
        Some(Some(original)) => set(conn, run, name, field, &original).await,
        Some(None) => refuse("the rule was added by the edits: it has no original value (remove it instead)"),
        None => refuse("this rule has no edit to undo"),
    }
}

async fn add(conn: &mut PgConnection, run: &mut Run<'_>, name: &str, fields: &Value) -> Result<()> {
    let Some(given) = fields.as_object() else { return refuse(format!("a new rule needs an object: {{\"{FIELD}\": {{…}}}}")) };
    if let Some(other) = given.keys().find(|k| k.as_str() != FIELD) {
        return refuse(format!("a rule has one field, {FIELD} (got {other:?})"));
    }
    let Some(value) = given.get(FIELD) else { return refuse(format!("a new rule needs the field {FIELD}")) };
    let new = user(validated(name, value))?;
    let s = run.s;
    if current(conn, s, name).await?.is_some() {
        return refuse("a rule of this name exists already in the editor's layer (set its rule, or remove it)");
    }
    if row_action(conn, s, TARGET, name).await?.as_deref() == Some("remove") {
        return refuse("this rule was removed by an earlier change: undo that change instead");
    }
    sqlx::query(sql(format!("INSERT INTO {}.rule_values (name, rule) VALUES ($1, $2)", s.content)))
        .bind(name)
        .bind(&new)
        .execute(&mut *conn)
        .await?;
    let row: Map<String, Value> = Map::from_iter([(FIELD.to_owned(), new)]);
    record_added(conn, run, TARGET, name, &row).await?;
    log(conn, run, TARGET, name, None, "add", None, Some(&Value::Object(row))).await?;
    run.effects.insert(Effect::Live);
    Ok(())
}

async fn remove(conn: &mut PgConnection, run: &mut Run<'_>, name: &str) -> Result<()> {
    let s = run.s;
    let Some(present) = current(conn, s, name).await? else { return refuse("there is no rule of this name in the editor's layer to remove") };
    let added = row_action(conn, s, TARGET, name).await?.as_deref() == Some("add");
    let edits: Vec<(String, Value, Option<Value>)> =
        sqlx::query_as(sql(format!("SELECT field, value, original FROM {}.fields WHERE target = $1 AND key = $2 ORDER BY field", s.edits)))
            .bind(TARGET)
            .bind(name)
            .fetch_all(&mut *conn)
            .await?;
    let snap = Snapshot {
        target: TARGET.to_owned(),
        key: name.to_owned(),
        added,
        row: Map::from_iter([(FIELD.to_owned(), present)]),
        edits: edits.into_iter().map(|(field, value, original)| FieldEdit { field, value, original }).collect(),
        children: Vec::new(),
    };
    sqlx::query(sql(format!("DELETE FROM {}.fields WHERE target = $1 AND key = $2", s.edits))).bind(TARGET).bind(name).execute(&mut *conn).await?;
    sqlx::query(sql(format!("DELETE FROM {}.rows WHERE target = $1 AND key = $2 AND action = 'add'", s.edits)))
        .bind(TARGET)
        .bind(name)
        .execute(&mut *conn)
        .await?;
    let json = serde_json::to_value(&snap)?;
    if !added {
        sqlx::query(sql(format!("INSERT INTO {}.rows (target, key, action, original, change_set) VALUES ($1, $2, 'remove', $3, $4)", s.edits)))
            .bind(TARGET)
            .bind(name)
            .bind(&json)
            .bind(run.cs)
            .execute(&mut *conn)
            .await?;
    }
    sqlx::query(sql(format!("DELETE FROM {}.rule_values WHERE name = $1", s.content))).bind(name).execute(&mut *conn).await?;
    log(conn, run, TARGET, name, None, "remove", Some(&json), None).await?;
    run.effects.insert(Effect::Live);
    Ok(())
}

async fn restore(conn: &mut PgConnection, run: &mut Run<'_>, snapshot: &Value) -> Result<()> {
    let snap: Snapshot = serde_json::from_value(snapshot.clone())?;
    let s = run.s;
    if current(conn, s, &snap.key).await?.is_some() {
        return refuse("a rule of this name exists already in the editor's layer");
    }
    let Some(value) = snap.row.get(FIELD) else { return refuse("the snapshot holds no rule") };
    sqlx::query(sql(format!("INSERT INTO {}.rule_values (name, rule) VALUES ($1, $2)", s.content)))
        .bind(&snap.key)
        .bind(value)
        .execute(&mut *conn)
        .await?;
    if snap.added {
        record_added(conn, run, TARGET, &snap.key, &snap.row).await?;
    } else {
        sqlx::query(sql(format!("DELETE FROM {}.rows WHERE target = $1 AND key = $2 AND action = 'remove'", s.edits)))
            .bind(TARGET)
            .bind(&snap.key)
            .execute(&mut *conn)
            .await?;
        for e in &snap.edits {
            sqlx::query(sql(format!(
                "INSERT INTO {}.fields (target, key, field, value, original, change_set) VALUES ($1, $2, $3, $4, $5, $6)",
                s.edits
            )))
            .bind(TARGET)
            .bind(&snap.key)
            .bind(&e.field)
            .bind(&e.value)
            .bind(&e.original)
            .bind(run.cs)
            .execute(&mut *conn)
            .await?;
        }
    }
    log(conn, run, TARGET, &snap.key, None, "restore", None, Some(snapshot)).await?;
    run.effects.insert(Effect::Live);
    Ok(())
}

/// `apply`: writes the rules the edits hold into `content.rule_values` (always: the `content` schema has no marker). A rule that
/// the game server would not accept any more is reported and left alone.
pub async fn apply_edits(conn: &mut PgConnection, s: &Schemas, report: &mut Report) -> Result<()> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL").bind(format!("{}.rule_values", s.content)).fetch_one(&mut *conn).await?;
    if !exists {
        report.missing.push(s.content.clone());
        return Ok(());
    }
    let removed: Vec<String> =
        sqlx::query_scalar(sql(format!("SELECT key FROM {}.rows WHERE target = $1 AND action = 'remove' ORDER BY key", s.edits)))
            .bind(TARGET)
            .fetch_all(&mut *conn)
            .await?;
    for name in removed {
        let gone = sqlx::query(sql(format!("DELETE FROM {}.rule_values WHERE name = $1", s.content))).bind(&name).execute(&mut *conn).await?;
        report.written += gone.rows_affected() as usize;
    }
    let fields: Vec<(String, Value, i32)> = sqlx::query_as(sql(format!(
        "SELECT key, value, change_set FROM {}.fields WHERE target = $1 AND field = $2 ORDER BY key",
        s.edits
    )))
    .bind(TARGET)
    .bind(FIELD)
    .fetch_all(&mut *conn)
    .await?;
    for (name, value, cs) in fields {
        let checked = key(&name).and_then(|n| validated(&n, &value));
        let rule = match checked {
            Ok(rule) => rule,
            Err(why) => {
                report.problem(Some(cs), TARGET, &name, Some(FIELD), format!("{why}; the rule was left alone"));
                continue;
            }
        };
        if current(conn, s, &name).await?.as_ref() != Some(&rule) {
            sqlx::query(sql(format!(
                "INSERT INTO {}.rule_values (name, rule) VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET rule = EXCLUDED.rule",
                s.content
            )))
            .bind(&name)
            .bind(&rule)
            .execute(&mut *conn)
            .await?;
            report.written += 1;
        }
    }
    Ok(())
}

/// `revert_all` for the rules: every one the edits added goes, every one they changed gets its original rule, every one they
/// removed comes back.
pub async fn revert(conn: &mut PgConnection, s: &Schemas) -> Result<()> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL").bind(format!("{}.rule_values", s.content)).fetch_one(&mut *conn).await?;
    if !exists {
        return Ok(());
    }
    let fields: Vec<(String, Option<Value>)> =
        sqlx::query_as(sql(format!("SELECT key, original FROM {}.fields WHERE target = $1 AND field = $2", s.edits)))
            .bind(TARGET)
            .bind(FIELD)
            .fetch_all(&mut *conn)
            .await?;
    for (name, original) in fields {
        match original.as_ref() {
            Some(rule) => {
                sqlx::query(sql(format!(
                    "INSERT INTO {}.rule_values (name, rule) VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET rule = EXCLUDED.rule",
                    s.content
                )))
                .bind(&name)
                .bind(rule)
                .execute(&mut *conn)
                .await?;
            }
            None => {
                sqlx::query(sql(format!("DELETE FROM {}.rule_values WHERE name = $1", s.content))).bind(&name).execute(&mut *conn).await?;
            }
        }
    }
    let removed: Vec<Value> =
        sqlx::query_scalar(sql(format!("SELECT original FROM {}.rows WHERE target = $1 AND action = 'remove' AND original IS NOT NULL", s.edits)))
            .bind(TARGET)
            .fetch_all(&mut *conn)
            .await?;
    for original in removed {
        let snap: Snapshot = serde_json::from_value(original)?;
        if let Some(value) = snap.row.get(FIELD) {
            sqlx::query(sql(format!("INSERT INTO {}.rule_values (name, rule) VALUES ($1, $2) ON CONFLICT (name) DO NOTHING", s.content)))
                .bind(&snap.key)
                .bind(value)
                .execute(&mut *conn)
                .await?;
        }
    }
    Ok(())
}
