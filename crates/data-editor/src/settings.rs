//! The `setting` target (`docs/plans/ext-1-foundation.md` section 7.4): the data editor's layer of the game server's settings. An
//! override is a row of `content.setting_values (name, value)`, key = the setting's name, one field `value` (text). Setting a value
//! that has no row adds one; removing the row falls back to the layers below (the config file, the extensions' defaults, the built-in
//! default). The change goes through the edits tables like every target (History, Undo, export and import work as usual) but the
//! table lives in the `content` schema, which no load replaces, so there is no marker: `apply` always writes it. Values are checked
//! by the game server's own parser (`game_server::config::check_setting`); a change takes effect at the next start.
//!
//! The keys of the other targets are numbers; this one is a name, so [`apply_line`] and the apply and revert functions here keep
//! clear of the numeric key code in `store` and `apply`.

use anyhow::Result;
use serde_json::{Map, Value};
use sqlx::{AssertSqlSafe, PgConnection};

use crate::apply::Report;
use crate::registry::{Check, Effect, Schemas};
use crate::store::{FieldEdit, Line, Op, Refused, Run, Snapshot, log, record_added, refuse, row_action, user};

/// The target's name in the edits tables.
pub const TARGET: &str = "setting";

/// The only field.
const FIELD: &str = "value";

fn sql(text: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(text)
}

/// `ext.<extension>.<option>` as its two parts: the editor's layer of an extension's option (`docs/plans/ext-5-lua.md` section 3.4).
pub fn ext_key(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("ext.")?;
    let (extension, option) = rest.split_once('.')?;
    (crate::ext::manifest::valid_name(extension) && game_server::script::options::valid_key(option)).then_some((extension, option))
}

/// A setting's name as a key: a game setting of the registry, or `ext.<extension>.<option>` (whether the extension is installed and
/// has the option is checked where the value is, against the database).
pub fn key(text: &str) -> Check<String> {
    let name = text.trim();
    if name.starts_with("ext.") {
        return if ext_key(name).is_some() {
            Ok(name.to_owned())
        } else {
            Err(format!("{name:?} is not an extension option (ext.<extension>.<option>, the option's key being a-z, 0-9 and _)"))
        };
    }
    if game_server::config::setting(name).is_none() {
        return Err(format!("{name:?} is not a game setting (docs/server/settings.md)"));
    }
    Ok(name.to_owned())
}

/// A value as the text the game server's parsers read: a string as written, a number or boolean as its text, a list joined by `,`
/// (the config file's rule, section 7.2).
pub fn value_text(value: &Value) -> Check<String> {
    Ok(match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => items
            .iter()
            .map(|v| match v {
                Value::String(s) => Ok(s.clone()),
                Value::Number(n) if n.is_i64() || n.is_u64() => Ok(n.to_string()),
                _ => Err("a list holds integers or strings".to_owned()),
            })
            .collect::<Check<Vec<_>>>()?
            .join(","),
        _ => return Err("a setting's value is a string, a number, a boolean or a list".into()),
    })
}

/// The text of `value` when the game server accepts it for the setting `name`. For an extension's option (`ext.<extension>.<option>`)
/// the text is the value as JSON (`5`, `"text"`, `[35,36]`), checked against the option's spec in `content.ext_options`; a user may
/// also give a number, a boolean or a list as text (`"5"`, `"[1,2]"`), which is read as JSON unless the option is a string.
async fn validated(conn: &mut PgConnection, s: &Schemas, name: &str, value: &Value) -> Result<Check<String>> {
    let Some((extension, option)) = ext_key(name) else { return Ok(plain(name, value)) };
    let spec = match option_spec(conn, s, extension, option).await? {
        Ok(spec) => spec,
        Err(why) => return Ok(Err(why)),
    };
    let typed = match value {
        Value::String(text) if spec.ty != game_server::script::OptType::String => match serde_json::from_str::<Value>(text) {
            Ok(v) => v,
            Err(_) => return Ok(Err(format!("{text:?} is not a {} (give JSON: a number, true, false or a list)", spec.ty.name()))),
        },
        other => other.clone(),
    };
    Ok(spec.check(&typed).map(|()| typed.to_string()).map_err(|e| format!("{e:#}")))
}

fn plain(name: &str, value: &Value) -> Check<String> {
    let text = value_text(value)?;
    game_server::config::check_setting(name, &text).map_err(|e| format!("{e:#}"))?;
    Ok(text)
}

/// The spec of an extension's option; the reason in `Err` when the extension or the option is not there.
async fn option_spec(
    conn: &mut PgConnection,
    s: &Schemas,
    extension: &str,
    option: &str,
) -> Result<Check<game_server::script::OptionSpec>> {
    let spec: Option<Value> =
        sqlx::query_scalar(sql(format!("SELECT spec FROM {}.ext_options WHERE extension = $1 AND name = $2", s.content)))
            .bind(extension)
            .bind(option)
            .fetch_optional(&mut *conn)
            .await?;
    if let Some(spec) = spec {
        return Ok(Ok(game_server::script::OptionSpec::parse(&spec)?));
    }
    let known: Vec<String> =
        sqlx::query_scalar(sql(format!("SELECT name FROM {}.ext_options WHERE extension = $1 ORDER BY name", s.content)))
            .bind(extension)
            .fetch_all(&mut *conn)
            .await?;
    let installed: bool = sqlx::query_scalar(sql(format!("SELECT EXISTS (SELECT 1 FROM {}.extensions WHERE name = $1)", s.content)))
        .bind(extension)
        .fetch_one(&mut *conn)
        .await?;
    Ok(Err(if !installed {
        format!("the extension {extension} is not installed")
    } else if known.is_empty() {
        format!("the extension {extension} has no options")
    } else {
        format!("the extension {extension} has no option {option:?} (its options: {})", known.join(", "))
    }))
}

/// When a change of the setting `name` takes effect: an extension's option at `reload`, the rest at the next start.
fn effect_of(name: &str) -> Effect {
    if ext_key(name).is_some() { Effect::Live } else { Effect::Restart }
}

/// The override now in the table.
async fn current(conn: &mut PgConnection, s: &Schemas, name: &str) -> Result<Option<String>> {
    Ok(sqlx::query_scalar(sql(format!("SELECT value FROM {}.setting_values WHERE name = $1", s.content)))
        .bind(name)
        .fetch_optional(&mut *conn)
        .await?)
}

/// Carries out one change of a setting override (inside the caller's transaction) and logs it.
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
        Ok(Refused(m)) => anyhow::Error::new(Refused(format!("setting {name}: {m}"))),
        Err(e) => e.context(format!("setting {name}")),
    })
}

fn only_field(field: &str) -> Result<()> {
    if field == FIELD { Ok(()) } else { refuse(format!("a setting has one field, {FIELD} (got {field:?})")) }
}

async fn set(conn: &mut PgConnection, run: &mut Run<'_>, name: &str, field: &str, value: &Value) -> Result<()> {
    only_field(field)?;
    let new = user(validated(conn, run.s, name, value).await?)?;
    set_text(conn, run, name, new).await
}

/// `set` with the text already validated (an `unset` puts the stored original back).
async fn set_text(conn: &mut PgConnection, run: &mut Run<'_>, name: &str, new: String) -> Result<()> {
    let s = run.s;
    let Some(present) = current(conn, s, name).await? else {
        // No override yet: setting a value makes one.
        return add_text(conn, run, name, new).await;
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
    let before = Value::String(present.clone());
    let after = Value::String(new.clone());
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
    sqlx::query(sql(format!("UPDATE {}.setting_values SET value = $2 WHERE name = $1", s.content)))
        .bind(name)
        .bind(&new)
        .execute(&mut *conn)
        .await?;
    log(conn, run, TARGET, name, Some(FIELD), action, Some(&before), Some(&after)).await?;
    run.effects.insert(effect_of(name));
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
        Some(Some(Value::String(original))) => set_text(conn, run, name, original).await,
        Some(Some(_)) => refuse("the stored original is not a value"),
        Some(None) => refuse("the override was added by the edits: it has no original value (remove it instead)"),
        None => refuse("this value has no edit to undo"),
    }
}

async fn add(conn: &mut PgConnection, run: &mut Run<'_>, name: &str, fields: &Value) -> Result<()> {
    let Some(given) = fields.as_object() else { return refuse(format!("a new override needs an object: {{\"{FIELD}\": …}}")) };
    if let Some(other) = given.keys().find(|k| k.as_str() != FIELD) {
        return refuse(format!("a setting has one field, {FIELD} (got {other:?})"));
    }
    let Some(value) = given.get(FIELD) else { return refuse(format!("a new override needs the field {FIELD}")) };
    let new = user(validated(conn, run.s, name, value).await?)?;
    add_text(conn, run, name, new).await
}

/// `add` with the text already validated.
async fn add_text(conn: &mut PgConnection, run: &mut Run<'_>, name: &str, new: String) -> Result<()> {
    let s = run.s;
    if current(conn, s, name).await?.is_some() {
        return refuse("an override of this setting exists already (set its value, or remove it)");
    }
    if row_action(conn, s, TARGET, name).await?.as_deref() == Some("remove") {
        return refuse("this override was removed by an earlier change: undo that change instead");
    }
    sqlx::query(sql(format!("INSERT INTO {}.setting_values (name, value) VALUES ($1, $2)", s.content)))
        .bind(name)
        .bind(&new)
        .execute(&mut *conn)
        .await?;
    let row: Map<String, Value> = Map::from_iter([(FIELD.to_owned(), Value::String(new))]);
    record_added(conn, run, TARGET, name, &row).await?;
    log(conn, run, TARGET, name, None, "add", None, Some(&Value::Object(row))).await?;
    run.effects.insert(effect_of(name));
    Ok(())
}

async fn remove(conn: &mut PgConnection, run: &mut Run<'_>, name: &str) -> Result<()> {
    let s = run.s;
    let Some(present) = current(conn, s, name).await? else { return refuse("there is no override of this setting to remove") };
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
        row: Map::from_iter([(FIELD.to_owned(), Value::String(present))]),
        edits: edits.into_iter().map(|(field, value, original)| FieldEdit { field, value, original }).collect(),
        children: Vec::new(),
    };
    sqlx::query(sql(format!("DELETE FROM {}.fields WHERE target = $1 AND key = $2", s.edits)))
        .bind(TARGET)
        .bind(name)
        .execute(&mut *conn)
        .await?;
    sqlx::query(sql(format!("DELETE FROM {}.rows WHERE target = $1 AND key = $2 AND action = 'add'", s.edits)))
        .bind(TARGET)
        .bind(name)
        .execute(&mut *conn)
        .await?;
    let json = serde_json::to_value(&snap)?;
    if !added {
        sqlx::query(sql(format!(
            "INSERT INTO {}.rows (target, key, action, original, change_set) VALUES ($1, $2, 'remove', $3, $4)",
            s.edits
        )))
        .bind(TARGET)
        .bind(name)
        .bind(&json)
        .bind(run.cs)
        .execute(&mut *conn)
        .await?;
    }
    sqlx::query(sql(format!("DELETE FROM {}.setting_values WHERE name = $1", s.content))).bind(name).execute(&mut *conn).await?;
    log(conn, run, TARGET, name, None, "remove", Some(&json), None).await?;
    run.effects.insert(effect_of(name));
    Ok(())
}

async fn restore(conn: &mut PgConnection, run: &mut Run<'_>, snapshot: &Value) -> Result<()> {
    let snap: Snapshot = serde_json::from_value(snapshot.clone())?;
    let s = run.s;
    if current(conn, s, &snap.key).await?.is_some() {
        return refuse("an override of this setting exists already");
    }
    let Some(value) = snap.row.get(FIELD).and_then(Value::as_str) else { return refuse("the snapshot holds no value") };
    sqlx::query(sql(format!("INSERT INTO {}.setting_values (name, value) VALUES ($1, $2)", s.content)))
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
    run.effects.insert(effect_of(&snap.key));
    Ok(())
}

/// `apply`: writes the overrides the edits hold into `content.setting_values` (always: the `content` schema has no marker). An
/// override that the game server would not accept any more is reported and left alone.
pub async fn apply_edits(conn: &mut PgConnection, s: &Schemas, report: &mut Report) -> Result<()> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(format!("{}.setting_values", s.content))
        .fetch_one(&mut *conn)
        .await?;
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
        let gone =
            sqlx::query(sql(format!("DELETE FROM {}.setting_values WHERE name = $1", s.content))).bind(&name).execute(&mut *conn).await?;
        report.written += gone.rows_affected() as usize;
    }
    let fields: Vec<(String, Value, i32)> =
        sqlx::query_as(sql(format!("SELECT key, value, change_set FROM {}.fields WHERE target = $1 AND field = $2 ORDER BY key", s.edits)))
            .bind(TARGET)
            .bind(FIELD)
            .fetch_all(&mut *conn)
            .await?;
    for (name, value, cs) in fields {
        let checked = match key(&name) {
            // An extension's option is stored as JSON text; the stored text is read back as the value it was.
            Ok(n) if ext_key(&n).is_some() => match value.as_str().map(serde_json::from_str::<Value>) {
                Some(Ok(typed)) => validated(conn, s, &n, &typed).await?,
                _ => Err("the stored value is not JSON".to_owned()),
            },
            Ok(n) => validated(conn, s, &n, &value).await?,
            Err(why) => Err(why),
        };
        let text = match checked {
            Ok(text) => text,
            Err(why) => {
                report.problem(Some(cs), TARGET, &name, Some(FIELD), format!("{why}; the override was left alone"));
                continue;
            }
        };
        if current(conn, s, &name).await?.as_deref() != Some(text.as_str()) {
            sqlx::query(sql(format!(
                "INSERT INTO {}.setting_values (name, value) VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value",
                s.content
            )))
            .bind(&name)
            .bind(&text)
            .execute(&mut *conn)
            .await?;
            report.written += 1;
        }
    }
    Ok(())
}

/// `revert_all` for the overrides: every one the edits added goes, every one they changed gets its original value, every one they
/// removed comes back.
pub async fn revert(conn: &mut PgConnection, s: &Schemas) -> Result<()> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(format!("{}.setting_values", s.content))
        .fetch_one(&mut *conn)
        .await?;
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
        match original.as_ref().and_then(Value::as_str) {
            Some(text) => {
                sqlx::query(sql(format!(
                    "INSERT INTO {}.setting_values (name, value) VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value",
                    s.content
                )))
                .bind(&name)
                .bind(text)
                .execute(&mut *conn)
                .await?;
            }
            None => {
                sqlx::query(sql(format!("DELETE FROM {}.setting_values WHERE name = $1", s.content)))
                    .bind(&name)
                    .execute(&mut *conn)
                    .await?;
            }
        }
    }
    let removed: Vec<Value> = sqlx::query_scalar(sql(format!(
        "SELECT original FROM {}.rows WHERE target = $1 AND action = 'remove' AND original IS NOT NULL",
        s.edits
    )))
    .bind(TARGET)
    .fetch_all(&mut *conn)
    .await?;
    for original in removed {
        let snap: Snapshot = serde_json::from_value(original)?;
        if let Some(value) = snap.row.get(FIELD).and_then(Value::as_str) {
            sqlx::query(sql(format!(
                "INSERT INTO {}.setting_values (name, value) VALUES ($1, $2) ON CONFLICT (name) DO NOTHING",
                s.content
            )))
            .bind(&snap.key)
            .bind(value)
            .execute(&mut *conn)
            .await?;
        }
    }
    Ok(())
}
