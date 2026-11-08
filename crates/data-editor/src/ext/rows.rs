//! The rows files of an extension (`docs/plans/ext-1-foundation.md` section 5.3): parsed without the database. The rows themselves are
//! computed by `ops.rs` against the frozen originals.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;
use toml::Table;

use super::tables::{self, ExtTable};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Item,
    Skill,
    Drop,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Item => "item",
            Kind::Skill => "skill",
            Kind::Drop => "drop",
        }
    }

    pub fn table(self) -> &'static ExtTable {
        match self {
            Kind::Item => tables::find("defs.items"),
            Kind::Skill => tables::find("defs.skills"),
            Kind::Drop => tables::find("defs.mob_items"),
        }
        .expect("the extendable tables")
    }

    /// The client-only fields of the kind, by name (the columns of `content.client_items` / `client_skills` after the look-alike).
    pub fn client_fields(self) -> &'static [&'static str] {
        match self {
            Kind::Item => &["model", "icon", "type2", "type3"],
            Kind::Skill => &["icon", "animation", "toggle_type", "sound"],
            Kind::Drop => &[],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Copy {
    pub from: String,
    pub from_key: Vec<i64>,
    pub columns: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClientSpec {
    /// `client = "keep"`: every client-only field, text included, from the record's own original.
    Keep,
    Fields {
        name: String,
        description: String,
        look: Option<(i64, i64)>,
        fields: BTreeMap<String, i64>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub kind: Kind,
    /// Its number among the entries of its kind, from 1.
    pub n: usize,
    pub file: String,
    pub id: String,
    /// The key columns' values: `[itemid]`, `[skillid, level]`, `[mobid, itemorder]`.
    pub key: Vec<i64>,
    pub copy: Option<Copy>,
    pub set: Vec<(String, Value)>,
    pub client: Option<ClientSpec>,
    pub server_only: Option<String>,
    pub note: Option<String>,
    /// An item's effect (`docs/plans/ext-3-effects.md` section 4.1), as written and checked by the server's parser.
    pub effect: Option<Value>,
    /// A skill level's abilities, as written and checked by the server's parser.
    pub abilities: Option<Value>,
}

impl Entry {
    /// `rows.toml [[item]] #2 (100/213)`, the start of every message about the entry.
    pub fn label(&self) -> String {
        format!("{} [[{}]] #{} ({})", self.file, self.kind.word(), self.n, self.id)
    }

    /// The key as the data editor's key text (`100212`, `200/1`, `2133/4`), as `content.ext_rows.key` holds it.
    pub fn key_text(&self) -> String {
        self.key.iter().map(i64::to_string).collect::<Vec<_>>().join("/")
    }
}

/// An id as written in a rows file and the key columns' values it stands for.
pub fn parse_id(kind: Kind, text: &str) -> Result<Vec<i64>, String> {
    let parts: Vec<&str> = text.split('/').collect();
    let number = |t: &str| t.trim().parse::<i64>().map_err(|_| format!("id {text:?} is not two numbers separated by /"));
    let [a, b] = parts[..] else { return Err(format!("id {text:?} is not two numbers separated by /")) };
    let (a, b) = (number(a)?, number(b)?);
    match kind {
        Kind::Item => {
            if !(1..=255).contains(&a) || !(1..=255).contains(&b) {
                return Err(format!("id {text:?}: type and typeid are numbers from 1 to 255"));
            }
            Ok(vec![a * 1000 + b])
        }
        Kind::Skill => {
            if !(1..=3).contains(&b) {
                return Err(format!("id {text:?}: the level is 1 to 3"));
            }
            Ok(vec![a, b])
        }
        Kind::Drop => {
            if !(1..=9).contains(&b) {
                return Err(format!("id {text:?}: the drop entry is 1 to 9"));
            }
            Ok(vec![a, b])
        }
    }
}

fn json(value: &toml::Value) -> Result<Value, String> {
    Ok(match value {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::from(*i),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f).map(Value::Number).ok_or("not a finite number")?,
        toml::Value::Boolean(b) => Value::Bool(*b),
        other => return Err(format!("a {} cannot be a column value", other.type_str())),
    })
}

/// Any TOML value as JSON (an effect's tables and lists); a date is not a value an effect holds.
fn toml_json(value: &toml::Value) -> Result<Value, String> {
    Ok(match value {
        toml::Value::Array(list) => Value::Array(list.iter().map(toml_json).collect::<Result<_, _>>()?),
        toml::Value::Table(table) => {
            Value::Object(table.iter().map(|(k, v)| Ok((k.clone(), toml_json(v)?))).collect::<Result<_, String>>()?)
        }
        other => json(other)?,
    })
}

fn parse_entry(kind: Kind, n: usize, file: &str, table: &Table) -> Result<Entry, String> {
    let mut entry = Entry {
        kind,
        n,
        file: file.to_owned(),
        id: String::new(),
        key: Vec::new(),
        copy: None,
        set: Vec::new(),
        client: None,
        server_only: None,
        note: None,
        effect: None,
        abilities: None,
    };
    let label = |e: &Entry| e.label();
    for key in table.keys() {
        let here = format!("{file} [[{}]] #{n}", kind.word());
        match (key.as_str(), kind) {
            ("effect", Kind::Skill) => return Err(format!("{here}: `effect` belongs to [[item]] (a [[skill]] has `abilities`)")),
            ("abilities", Kind::Item) => return Err(format!("{here}: `abilities` belongs to [[skill]] (an [[item]] has `effect`)")),
            ("effect", Kind::Item) | ("abilities", Kind::Skill) => {}
            _ if ["id", "copy", "set", "client", "server_only", "note"].contains(&key.as_str()) => {}
            _ => return Err(format!("{here}: unknown key `{key}`")),
        }
    }
    let id = table
        .get("id")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{file} [[{}]] #{n}: the required key `id` is missing", kind.word()))?;
    entry.id = id.to_owned();
    entry.key = parse_id(kind, id).map_err(|e| format!("{}: {e}", label(&entry)))?;
    if let Some(value) = table.get("copy") {
        let copy = value.as_table().ok_or_else(|| format!("{}: `copy` must be {{ from = \"…\", columns = [..] }}", label(&entry)))?;
        for key in copy.keys() {
            if !["from", "columns"].contains(&key.as_str()) {
                return Err(format!("{}: unknown key `{key}` in copy", label(&entry)));
            }
        }
        let from = copy.get("from").and_then(toml::Value::as_str).ok_or_else(|| format!("{}: copy needs `from`", label(&entry)))?;
        let from_key = parse_id(kind, from).map_err(|e| format!("{}: copy from: {e}", label(&entry)))?;
        let columns = match copy.get("columns") {
            None => None,
            Some(toml::Value::Array(list)) => Some(
                list.iter()
                    .map(|v| v.as_str().map(str::to_owned).ok_or_else(|| format!("{}: copy columns must be strings", label(&entry))))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Some(_) => return Err(format!("{}: copy columns must be a list", label(&entry))),
        };
        entry.copy = Some(Copy { from: from.to_owned(), from_key, columns });
    }
    if let Some(value) = table.get("set") {
        let set = value.as_table().ok_or_else(|| format!("{}: `set` must be a table of columns", label(&entry)))?;
        for (column, v) in set {
            entry.set.push((column.clone(), json(v).map_err(|e| format!("{}: set {column}: {e}", label(&entry)))?));
        }
    }
    match (table.get("client"), table.get("server_only")) {
        (Some(_), Some(_)) => return Err(format!("{}: `client` and `server_only` exclude each other", label(&entry))),
        (Some(_), None) if kind == Kind::Drop => {
            return Err(format!("{}: a drop entry has no client row (the client does not show drops)", label(&entry)));
        }
        (None, Some(_)) if kind == Kind::Drop => {
            return Err(format!("{}: a drop entry has no client row, so no server_only reason either", label(&entry)));
        }
        _ => {}
    }
    if let Some(value) = table.get("client") {
        entry.client = Some(parse_client(kind, value).map_err(|e| format!("{}: client: {e}", label(&entry)))?);
    }
    if let Some(value) = table.get("server_only") {
        entry.server_only = Some(
            value
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| format!("{}: server_only must be the reason, a text", label(&entry)))?
                .to_owned(),
        );
    }
    if let Some(value) = table.get("effect") {
        let json = toml_json(value).map_err(|e| format!("{}: effect: {e}", label(&entry)))?;
        game_server::effects::check_effect(&json).map_err(|e| format!("{}: effect: {e}", label(&entry)))?;
        entry.effect = Some(json);
    }
    if let Some(value) = table.get("abilities") {
        let json = toml_json(value).map_err(|e| format!("{}: abilities: {e}", label(&entry)))?;
        game_server::effects::check_abilities(&json).map_err(|e| format!("{}: abilities: {e}", label(&entry)))?;
        entry.abilities = Some(json);
    }
    if let Some(value) = table.get("note") {
        entry.note = Some(value.as_str().ok_or_else(|| format!("{}: note must be a text", label(&entry)))?.to_owned());
    }
    Ok(entry)
}

fn parse_client(kind: Kind, value: &toml::Value) -> Result<ClientSpec, String> {
    if value.as_str() == Some("keep") {
        return Ok(ClientSpec::Keep);
    }
    let table = value.as_table().ok_or("must be \"keep\" or a table { name, description, look, … }")?;
    let allowed: Vec<&str> = ["name", "description", "look"].into_iter().chain(kind.client_fields().iter().copied()).collect();
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("unknown key `{key}` (a {} may give: {})", kind.word(), allowed.join(", ")));
        }
    }
    let text =
        |key: &str| table.get(key).and_then(toml::Value::as_str).map(str::to_owned).ok_or_else(|| format!("`{key}` is required (a text)"));
    let look = match table.get("look") {
        None => None,
        Some(v) => {
            let text = v.as_str().ok_or("`look` must be a client id like \"100/108\"")?;
            let (a, b) = text.split_once('/').ok_or_else(|| format!("look {text:?} must be a client id like \"100/108\""))?;
            let number = |t: &str| t.trim().parse::<i64>().map_err(|_| format!("look {text:?} must be a client id like \"100/108\""));
            Some((number(a)?, number(b)?))
        }
    };
    let mut fields = BTreeMap::new();
    for field in kind.client_fields() {
        if let Some(v) = table.get(*field) {
            fields.insert((*field).to_owned(), v.as_integer().ok_or_else(|| format!("`{field}` must be a whole number"))?);
        }
    }
    Ok(ClientSpec::Fields { name: text("name")?, description: text("description")?, look, fields })
}

/// The fields of a `Monster.SData` record that a `[[monster_look]]` may set, with the client's widths (`client_sdata.MONSTER_FIELDS`; a test
/// compares the names). `name` (printable ASCII) is the one more.
pub const MONSTER_FIELDS: &[(&str, i64, i64)] = &[
    ("model", -32768, 32767),
    ("level", -32768, 32767),
    ("ai", 0, 255),
    ("hp", i32::MIN as i64, i32::MAX as i64),
    ("day", 0, 255),
    ("size", 0, 255),
    ("element", 0, 255),
    ("normal_time", i32::MIN as i64, i32::MAX as i64),
    ("normal_step", 0, 255),
    ("chase_time", i32::MIN as i64, i32::MAX as i64),
    ("chase_step", 0, 255),
    ("attack_type1", 0, 255),
    ("attack_ani1", 0, 255),
    ("attack_type2", 0, 255),
    ("attack_ani2", 0, 255),
    ("attack_type3", 0, 255),
    ("attack_ani3", 0, 255),
    ("attack_plus3", 0, 255),
    ("quest_item", -32768, 32767),
];

/// A `[[monster_look]]` entry (`docs/plans/ext-4-pets.md` section 6.4): client-only, no `defs` row. The record `base + m` is the client's
/// record `m` with the day mapped and the fields set; it expands into rows of `content.client_monsters`.
#[derive(Clone, Debug, PartialEq)]
pub struct MonsterLook {
    pub file: String,
    /// Its number among the `[[monster_look]]` entries of the extension, from 1.
    pub n: usize,
    /// The client records copied, as inclusive ranges in the order written.
    pub of: Vec<(i64, i64)>,
    pub base: i64,
    /// The copy's Day by the original's (both 0 to 255); an original day not named is kept.
    pub day: BTreeMap<i64, i64>,
    /// The fields set, by name (`name` included), validated against their widths.
    pub set: BTreeMap<String, Value>,
    pub note: String,
}

impl MonsterLook {
    pub fn label(&self) -> String {
        format!("{} [[monster_look]] #{}", self.file, self.n)
    }

    /// The copied record ids, ascending.
    pub fn ids(&self) -> Vec<i64> {
        let mut ids: Vec<i64> = self.of.iter().flat_map(|&(low, high)| low..=high).collect();
        ids.sort_unstable();
        ids
    }

    /// The fields of every row it makes: `set`, and `day` as the map when there is one (the generator maps it by the copied record's day,
    /// so no `client` schema is needed here).
    pub fn fields(&self) -> Value {
        let mut fields: serde_json::Map<String, Value> = self.set.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        if !self.day.is_empty() {
            fields
                .insert("day".to_owned(), Value::Object(self.day.iter().map(|(from, to)| (from.to_string(), Value::from(*to))).collect()));
        }
        Value::Object(fields)
    }

    /// The one line `ext show` and `ext check` print: `monster_look 1-2473 → 3001-5473, day mapped, 6 fields set`.
    pub fn summary(&self) -> String {
        let ids = self.ids();
        monster_look_line(ids.first().copied(), ids.last().copied(), ids.len(), self.base, !self.day.is_empty(), self.set.len())
    }
}

/// The line of a group of records `first..=last` (`count` of them) copied to `first + base`, with the day mapped or not and `fields` set.
pub fn monster_look_line(first: Option<i64>, last: Option<i64>, count: usize, base: i64, day: bool, fields: usize) -> String {
    let (Some(first), Some(last)) = (first, last) else { return "monster_look (no records)".to_owned() };
    let span = |a: i64, b: i64| if a == b { a.to_string() } else { format!("{a}-{b}") };
    let holes = if usize::try_from(last - first + 1).is_ok_and(|n| n != count) { format!(" ({count} records)") } else { String::new() };
    format!(
        "monster_look {} → {}{holes}, {}, {fields} field{} set",
        span(first, last),
        span(base + first, base + last),
        if day { "day mapped" } else { "day kept" },
        if fields == 1 { "" } else { "s" }
    )
}

/// `of`: one id, a text `"600-620"`, or a list of both (`[1234, "600-620"]`), each id 0 to 2,473.
fn parse_id_set(value: &toml::Value) -> Result<Vec<(i64, i64)>, String> {
    let one = |v: &toml::Value| -> Result<(i64, i64), String> {
        let bad = || format!("{v} is not a record id or range (a number, or \"600-620\")");
        let (low, high) = match v {
            toml::Value::Integer(n) => (*n, *n),
            toml::Value::String(t) => match t.split_once('-') {
                Some((a, b)) => (a.trim().parse().map_err(|_| bad())?, b.trim().parse().map_err(|_| bad())?),
                None => {
                    let n = t.trim().parse().map_err(|_| bad())?;
                    (n, n)
                }
            },
            _ => return Err(bad()),
        };
        if low > high {
            return Err(format!("{v}: the range runs backwards"));
        }
        if low < 0 || high >= super::ids::MONSTER_ORIGINALS {
            return Err(format!("{v}: the client's Monster.SData holds records 0 to {}", super::ids::MONSTER_ORIGINALS - 1));
        }
        Ok((low, high))
    };
    let ranges = match value {
        toml::Value::Array(list) => list.iter().map(one).collect::<Result<Vec<_>, _>>()?,
        other => vec![one(other)?],
    };
    if ranges.is_empty() {
        return Err("lists no records".to_owned());
    }
    let mut sorted = ranges.clone();
    sorted.sort_unstable();
    if let Some(pair) = sorted.windows(2).find(|w| w[1].0 <= w[0].1) {
        return Err(format!("records {}-{} and {}-{} overlap", pair[0].0, pair[0].1, pair[1].0, pair[1].1));
    }
    Ok(ranges)
}

fn parse_monster_look(n: usize, file: &str, table: &Table) -> Result<MonsterLook, String> {
    let here = format!("{file} [[monster_look]] #{n}");
    for key in table.keys() {
        if !["of", "base", "day", "set", "note"].contains(&key.as_str()) {
            return Err(format!("{here}: unknown key `{key}` (a monster_look has of, base, day, set and note)"));
        }
    }
    let of = parse_id_set(table.get("of").ok_or_else(|| format!("{here}: the required key `of` is missing"))?)
        .map_err(|e| format!("{here}: of: {e}"))?;
    let base = table
        .get("base")
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("{here}: the required key `base` is missing (a whole number: the copy of record m is record base + m)"))?;
    if !(super::ids::SERVER_MONSTERS..=super::ids::MONSTER_ID_MAX).contains(&base) {
        return Err(format!(
            "{here}: base {base}: a record's id is {} to {} (after the server's monsters, up to the client's u16)",
            super::ids::SERVER_MONSTERS,
            super::ids::MONSTER_ID_MAX
        ));
    }
    let mut day = BTreeMap::new();
    if let Some(value) = table.get("day") {
        let map =
            value.as_table().ok_or_else(|| format!("{here}: day must be a table {{ \"0\" = 6, … }} (the copy's Day by the original's)"))?;
        for (from, to) in map {
            let from: i64 = from
                .parse()
                .ok()
                .filter(|n| (0..=255).contains(n))
                .ok_or_else(|| format!("{here}: day: the key {from:?} is not 0 to 255"))?;
            let to =
                to.as_integer().filter(|n| (0..=255).contains(n)).ok_or_else(|| format!("{here}: day: {from} = {to} is not 0 to 255"))?;
            day.insert(from, to);
        }
    }
    let mut set = BTreeMap::new();
    if let Some(value) = table.get("set") {
        let fields = value.as_table().ok_or_else(|| format!("{here}: set must be a table of Monster.SData fields"))?;
        for (name, v) in fields {
            if name == "name" {
                let text = v.as_str().ok_or_else(|| format!("{here}: set name: must be a text"))?;
                if !text.bytes().all(|b| (0x20..=0x7E).contains(&b)) {
                    return Err(format!("{here}: set name: {text:?} is not printable ASCII (0x20 to 0x7E)"));
                }
                set.insert(name.clone(), Value::String(text.to_owned()));
                continue;
            }
            let Some(&(_, low, high)) = MONSTER_FIELDS.iter().find(|(f, _, _)| f == name) else {
                let names: Vec<&str> = MONSTER_FIELDS.iter().map(|(f, _, _)| *f).collect();
                return Err(format!("{here}: set {name}: not a field of Monster.SData ({}, name)", names.join(", ")));
            };
            let number = v.as_integer().ok_or_else(|| format!("{here}: set {name}: {v} is not a whole number"))?;
            if !(low..=high).contains(&number) {
                return Err(format!("{here}: set {name}: {number} is not {low} to {high} (the client's field)"));
            }
            if name == "day" && !day.is_empty() {
                return Err(format!("{here}: day is both the `day` map and in `set`; use one"));
            }
            set.insert(name.clone(), Value::from(number));
        }
    }
    let note = match table.get("note") {
        None => String::new(),
        Some(v) => v.as_str().ok_or_else(|| format!("{here}: note must be a text"))?.to_owned(),
    };
    Ok(MonsterLook { file: file.to_owned(), n, of, base, day, set, note })
}

/// The `[[monster_look]]` entries of one rows file, in file order.
pub fn parse_monster_looks(file: &str, text: &str) -> Result<Vec<MonsterLook>, String> {
    let table: Table = toml::from_str(text).map_err(|e| format!("{file}: not valid TOML: {}", e.to_string().trim_end()))?;
    match table.get("monster_look") {
        None => Ok(Vec::new()),
        Some(toml::Value::Array(list)) => list
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let t = v.as_table().ok_or_else(|| format!("{file}: [[monster_look]] #{} is not a table", i + 1))?;
                parse_monster_look(i + 1, file, t)
            })
            .collect(),
        Some(_) => Err(format!("{file}: `monster_look` must be an array of tables ([[monster_look]])")),
    }
}

/// The entries of one rows file, in file order within each kind.
pub fn parse_rows(file: &str, text: &str) -> Result<Vec<Entry>, String> {
    let table: Table = toml::from_str(text).map_err(|e| format!("{file}: not valid TOML: {}", e.to_string().trim_end()))?;
    let mut out = Vec::new();
    for key in table.keys() {
        if !["item", "skill", "drop", "monster_look"].contains(&key.as_str()) {
            return Err(format!(
                "{file}: unknown key `{key}` (a rows file holds [[item]], [[skill]], [[drop]] and [[monster_look]] entries)"
            ));
        }
    }
    for kind in [Kind::Item, Kind::Skill, Kind::Drop] {
        match table.get(kind.word()) {
            None => {}
            Some(toml::Value::Array(list)) => {
                for (i, v) in list.iter().enumerate() {
                    let t = v.as_table().ok_or_else(|| format!("{file}: [[{}]] #{} is not a table", kind.word(), i + 1))?;
                    out.push(parse_entry(kind, i + 1, file, t)?);
                }
            }
            Some(_) => return Err(format!("{file}: `{}` must be an array of tables ([[{}]])", kind.word(), kind.word())),
        }
    }
    Ok(out)
}

/// `rows.toml`, then `rows/*.toml` by file name, from the extension's folder: all entries ordered items, skills, drops, each in file order
/// (that order is `seq` in `content.ext_rows`, and the order rows are written in).
pub fn load_rows(dir: &Path) -> Result<Vec<Entry>, String> {
    let files = rows_files(dir)?;
    let mut all = Vec::new();
    let mut counts = BTreeMap::new();
    for (name, path) in files {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{name}: {e}"))?;
        for mut entry in parse_rows(&name, &text)? {
            // numbering runs on across files, so `#3` is unique within the extension
            let n = counts.entry(entry.kind).or_insert(0usize);
            *n += 1;
            entry.n = *n;
            all.push(entry);
        }
    }
    all.sort_by_key(|e| (e.kind, e.n));
    Ok(all)
}

/// The `[[monster_look]]` entries of `rows.toml` and `rows/*.toml`, numbered across the files; no two may make the same record.
pub fn load_monster_looks(dir: &Path) -> Result<Vec<MonsterLook>, String> {
    let mut all: Vec<MonsterLook> = Vec::new();
    for (name, path) in rows_files(dir)? {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{name}: {e}"))?;
        for mut look in parse_monster_looks(&name, &text)? {
            look.n = all.len() + 1;
            all.push(look);
        }
    }
    let mut made: BTreeMap<i64, usize> = BTreeMap::new();
    for look in &all {
        for m in look.ids() {
            let id = look.base + m;
            if id > super::ids::MONSTER_ID_MAX {
                return Err(format!("{}: record {m} + base {} is {id}, over {}", look.label(), look.base, super::ids::MONSTER_ID_MAX));
            }
            if let Some(first) = made.insert(id, look.n) {
                return Err(format!("{}: makes record {id} (a copy of {m}) that [[monster_look]] #{first} makes already", look.label()));
            }
        }
    }
    Ok(all)
}

/// `rows.toml`, then `rows/*.toml` by file name: (the name shown in messages, the path).
fn rows_files(dir: &Path) -> Result<Vec<(String, std::path::PathBuf)>, String> {
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    if dir.join("rows.toml").is_file() {
        files.push(("rows.toml".into(), dir.join("rows.toml")));
    }
    if dir.join("rows").is_dir() {
        let mut names: Vec<_> = std::fs::read_dir(dir.join("rows"))
            .map_err(|e| format!("{}: {e}", dir.join("rows").display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        names.sort();
        for p in names {
            files.push((format!("rows/{}", p.file_name().unwrap_or_default().to_string_lossy()), p));
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOOK: &str = r#"
[[monster_look]]
of = "1-2473"
base = 3000
day = { "0" = 6, "1" = 6, "2" = 0, "3" = 0, "4" = 1, "5" = 4, "6" = 6, "7" = 6 }
set = { size = 0, hp = 1, quest_item = 0, normal_time = 1000, normal_step = 4, chase_time = 1000, chase_step = 10 }
note = "pets: small, unclickable copies"
"#;

    #[test]
    fn a_monster_look_parses_and_summarises_as_one_line() {
        let looks = parse_monster_looks("rows.toml", LOOK).unwrap();
        assert_eq!(looks.len(), 1);
        let look = &looks[0];
        assert_eq!((look.of.as_slice(), look.base, look.note.as_str()), ([(1, 2473)].as_slice(), 3000, "pets: small, unclickable copies"));
        assert_eq!(look.ids().len(), 2473);
        assert_eq!(look.summary(), "monster_look 1-2473 → 3001-5473, day mapped, 7 fields set");
        let fields = look.fields();
        assert_eq!(fields["size"], 0);
        assert_eq!(fields["chase_step"], 10);
        assert_eq!(fields["day"]["4"], 1, "the day map travels in the fields, resolved by the generator");
        assert_eq!(look.label(), "rows.toml [[monster_look]] #1");
        // a rows file with only monster looks has no item entries
        assert!(parse_rows("rows.toml", LOOK).unwrap().is_empty());
    }

    #[test]
    fn of_takes_numbers_texts_and_lists() {
        let of =
            |text: &str| parse_monster_looks("r.toml", &format!("[[monster_look]]\nof = {text}\nbase = 3000\n")).map(|l| l[0].of.clone());
        assert_eq!(of("5").unwrap(), [(5, 5)]);
        assert_eq!(of("\"600-620\"").unwrap(), [(600, 620)]);
        assert_eq!(of("[1234, \"600-620\", 7]").unwrap(), [(1234, 1234), (600, 620), (7, 7)]);
        for (text, needle) in [
            ("\"0-2474\"", "holds records 0 to 2473"),
            ("-1", "holds records 0 to 2473"),
            ("\"9-3\"", "runs backwards"),
            ("[1, \"1-5\"]", "overlap"),
            ("[]", "lists no records"),
            ("\"a\"", "is not a record id or range"),
            ("1.5", "is not a record id or range"),
            ("true", "is not a record id or range"),
        ] {
            let error = of(text).unwrap_err();
            assert!(error.contains(needle) && error.starts_with("r.toml [[monster_look]] #1: of: "), "{text}: {error}");
        }
    }

    #[test]
    fn a_monster_look_is_checked_key_by_key() {
        let base = |extra: &str| format!("[[monster_look]]\nof = \"1-5\"\nbase = 3000\n{extra}\n");
        let error = |extra: &str| parse_monster_looks("rows.toml", &base(extra)).unwrap_err();
        assert!(parse_monster_looks("rows.toml", &base("")).is_ok());
        for (extra, needle) in [
            ("size = 0", "unknown key `size`"),
            ("set = { size = 256 }", "set size: 256 is not 0 to 255"),
            ("set = { size = -1 }", "set size: -1 is not 0 to 255"),
            ("set = { model = 40000 }", "set model: 40000 is not -32768 to 32767"),
            ("set = { hp = 3000000000 }", "set hp: 3000000000 is not"),
            ("set = { size = \"big\" }", "set size: \"big\" is not a whole number"),
            ("set = { colour = 1 }", "set colour: not a field of Monster.SData"),
            ("set = { name = 5 }", "set name: must be a text"),
            ("set = { name = \"Naïve\" }", "not printable ASCII"),
            ("set = 1", "set must be a table"),
            ("day = { \"256\" = 1 }", "the key \"256\" is not 0 to 255"),
            ("day = { x = 1 }", "the key \"x\" is not 0 to 255"),
            ("day = { \"1\" = 300 }", "1 = 300 is not 0 to 255"),
            ("day = 3", "day must be a table"),
            ("day = { \"1\" = 6 }\nset = { day = 2 }", "both the `day` map and in `set`"),
            ("note = 5", "note must be a text"),
        ] {
            let e = error(extra);
            assert!(e.contains(needle) && e.starts_with("rows.toml [[monster_look]] #1: "), "{extra}: {e}");
        }
        assert!(parse_monster_looks("rows.toml", "[[monster_look]]\nbase = 3000\n").unwrap_err().contains("required key `of` is missing"));
        assert!(parse_monster_looks("rows.toml", "[[monster_look]]\nof = 1\n").unwrap_err().contains("required key `base` is missing"));
        for base in ["2746", "65536", "0"] {
            let e = parse_monster_looks("rows.toml", &format!("[[monster_look]]\nof = 1\nbase = {base}\n")).unwrap_err();
            assert!(e.contains("a record's id is 2747 to 65535"), "{base}: {e}");
        }
        assert!(parse_monster_looks("rows.toml", "monster_look = 1").unwrap_err().contains("must be an array of tables"));
        assert!(parse_rows("rows.toml", "[[mobs]]\n").unwrap_err().contains("[[monster_look]] entries"));
    }

    #[test]
    fn the_line_names_what_it_copies() {
        assert_eq!(
            monster_look_line(Some(1), Some(2473), 2473, 3000, true, 6),
            "monster_look 1-2473 → 3001-5473, day mapped, 6 fields set"
        );
        assert_eq!(monster_look_line(Some(5), Some(5), 1, 3000, false, 1), "monster_look 5 → 3005, day kept, 1 field set");
        assert_eq!(
            monster_look_line(Some(1), Some(10), 4, 100, false, 0),
            "monster_look 1-10 → 101-110 (4 records), day kept, 0 fields set"
        );
        assert_eq!(monster_look_line(None, None, 0, 3000, false, 0), "monster_look (no records)");
    }

    #[test]
    fn the_settable_fields_are_the_client_files_fields() {
        let python = include_str!("../../../../tools/sdata/client_sdata.py");
        let block =
            python.split("MONSTER_FIELDS = [").nth(1).and_then(|t| t.split("\n]\n").next()).expect("MONSTER_FIELDS in client_sdata.py");
        let mut want: Vec<(String, char)> = Vec::new();
        for part in block.split("(\"").skip(1) {
            let name = part.split('"').next().unwrap();
            let fmt = part.split("\"").nth(2).unwrap().chars().next().unwrap();
            want.push((name.to_owned(), fmt));
        }
        assert_eq!(want.len(), 19);
        assert_eq!(MONSTER_FIELDS.len(), want.len());
        for ((name, low, high), (py_name, fmt)) in MONSTER_FIELDS.iter().zip(&want) {
            assert_eq!(name, py_name);
            let range = match fmt {
                'B' => (0, 255),
                'h' => (-32768, 32767),
                'i' => (i32::MIN as i64, i32::MAX as i64),
                other => panic!("format {other}"),
            };
            assert_eq!((*low, *high), range, "{name}");
        }
    }

    const ROWS: &str = r#"
[[skill]]
id = "200/1"
copy = { from = "250/1" }
set = { skillname = "Kill Boost", keeptime = 1 }
client = { name = "Kill Boost", look = "250/1", description = "d" }
note = "n"

[[item]]
id = "100/212"
copy = { from = "100/108", columns = ["reqlevel", "buy"] }
set = { itemname = "Stone", range = 200 }
client = { name = "Stone", look = "100/108", description = "d", icon = 5 }

[[item]]
id = "100/213"
server_only = "a reason"

[[drop]]
id = "2133/4"
set = { grade = 3, droprate = 50 }
"#;

    #[test]
    fn rows_parse_in_kind_order() {
        let rows = parse_rows("rows.toml", ROWS).unwrap();
        let ids: Vec<_> = rows.iter().map(|e| (e.kind.word(), e.id.as_str())).collect();
        assert_eq!(ids, [("item", "100/212"), ("item", "100/213"), ("skill", "200/1"), ("drop", "2133/4")]);
        let stone = &rows[0];
        assert_eq!(stone.key, [100212]);
        assert_eq!(stone.key_text(), "100212");
        assert_eq!(stone.copy.as_ref().unwrap().from_key, [100108]);
        assert_eq!(stone.copy.as_ref().unwrap().columns.as_deref(), Some(&["reqlevel".to_owned(), "buy".to_owned()][..]));
        assert_eq!(stone.set, [("itemname".to_owned(), Value::from("Stone")), ("range".to_owned(), Value::from(200))]);
        match stone.client.as_ref().unwrap() {
            ClientSpec::Fields { name, look, fields, .. } => {
                assert_eq!((name.as_str(), *look), ("Stone", Some((100, 108))));
                assert_eq!(fields["icon"], 5);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(rows[1].server_only.as_deref(), Some("a reason"));
        assert_eq!(rows[2].key, [200, 1]);
        assert_eq!(rows[2].label(), "rows.toml [[skill]] #1 (200/1)");
        assert_eq!(rows[3].key_text(), "2133/4");
    }

    #[test]
    fn keep_is_accepted() {
        let rows = parse_rows("rows.toml", "[[item]]\nid = \"100/212\"\nclient = \"keep\"\n").unwrap();
        assert_eq!(rows[0].client, Some(ClientSpec::Keep));
    }

    #[test]
    fn mistakes_name_the_file_the_entry_and_the_key() {
        let one = |kind: &str, body: &str| parse_rows("rows.toml", &format!("[[{kind}]]\n{body}")).unwrap_err();
        let cases = [
            (one("item", "id = \"100/212\"\nwat = 1"), "rows.toml [[item]] #1: unknown key `wat`"),
            (one("item", "set = {}"), "rows.toml [[item]] #1: the required key `id` is missing"),
            (one("item", "id = \"100\""), "id \"100\" is not two numbers"),
            (one("skill", "id = \"200/4\""), "the level is 1 to 3"),
            (one("drop", "id = \"2133/10\""), "the drop entry is 1 to 9"),
            (one("item", "id = \"100/212\"\nclient = \"keep\"\nserver_only = \"x\""), "exclude each other"),
            (one("drop", "id = \"2133/4\"\nclient = \"keep\""), "a drop entry has no client row"),
            (one("item", "id = \"100/212\"\nclient = { name = \"a\", description = \"b\", flavour = 1 }"), "client: unknown key `flavour`"),
            (one("item", "id = \"100/212\"\nclient = { name = \"a\" }"), "`description` is required"),
            (
                one("skill", "id = \"200/1\"\nclient = { name = \"a\", description = \"b\", model = 1 }"),
                "unknown key `model` (a skill may give",
            ),
            (one("item", "id = \"100/212\"\ncopy = { from = \"100/108\", wat = 1 }"), "unknown key `wat` in copy"),
            (one("item", "id = \"100/212\"\ncopy = { columns = [\"a\"] }"), "copy needs `from`"),
            (one("item", "id = \"100/212\"\nset = { a = [1] }"), "set a: a array cannot be a column value"),
            (parse_rows("rows.toml", "[[monster]]\nid = \"1/1\"").unwrap_err(), "unknown key `monster`"),
            (parse_rows("rows.toml", "item = 1").unwrap_err(), "must be an array of tables"),
            (parse_rows("rows.toml", "[[item").unwrap_err(), "not valid TOML"),
        ];
        for (error, needle) in cases {
            assert!(error.contains(needle), "wanted {needle:?}, got {error:?}");
        }
    }

    #[test]
    fn effects_and_abilities_parse_with_their_entry_in_every_error() {
        let rows = parse_rows(
            "rows.toml",
            "[[item]]\nid = \"100/215\"\neffect = { type = \"level_up\", levels = 1 }\n[[skill]]\nid = \"200/1\"\nabilities = [{ type = \"kill_boost\", times = 2 }]\n",
        )
        .unwrap();
        assert_eq!(rows[0].effect, Some(serde_json::json!({ "type": "level_up", "levels": 1 })));
        assert_eq!(rows[1].abilities, Some(serde_json::json!([{ "type": "kill_boost", "times": 2 }])));
        // Nested tables and lists come through as JSON.
        let rows =
            parse_rows("rows.toml", "[[item]]\nid = \"95/3\"\neffect = { type = \"enchant\", jumps = [[1, 60], [2, 40]], safe = true }\n")
                .unwrap();
        assert_eq!(rows[0].effect.as_ref().unwrap()["jumps"], serde_json::json!([[1, 60], [2, 40]]));
        let one = |kind: &str, body: &str| parse_rows("rows.toml", &format!("[[{kind}]]\n{body}")).unwrap_err();
        let cases = [
            (
                one("item", "id = \"100/215\"\neffect = { type = \"level_up\", levels = 0 }"),
                "rows.toml [[item]] #1 (100/215): effect: levels: 0 is not 1 to 59",
            ),
            (one("item", "id = \"100/215\"\neffect = { type = \"level_up\" }"), "(100/215): effect: a level_up effect needs one of"),
            (one("item", "id = \"100/215\"\neffect = \"level_up\""), "effect: an effect is a table"),
            (one("item", "id = \"100/215\"\neffect = { type = \"nothing\" }"), "effect: type: unknown type \"nothing\""),
            (
                one("item", "id = \"100/215\"\neffect = { type = \"level_up\", levels = 1, rate = 3 }"),
                "effect: unknown key \"rate\" (not a key of the level_up effect)",
            ),
            (
                one("skill", "id = \"200/1\"\nabilities = [{ type = \"kill_boost\", times = 101 }]"),
                "rows.toml [[skill]] #1 (200/1): abilities: ability 1: times: 101 is not 1 to 100",
            ),
            (one("skill", "id = \"200/1\"\nabilities = []"), "abilities: 0 abilities is not 1 to 4"),
            (one("skill", "id = \"200/1\"\neffect = { type = \"level_up\", levels = 1 }"), "`effect` belongs to [[item]]"),
            (one("item", "id = \"100/215\"\nabilities = [{ type = \"kill_boost\", times = 2 }]"), "`abilities` belongs to [[skill]]"),
            (one("drop", "id = \"2133/4\"\neffect = { type = \"level_up\", levels = 1 }"), "unknown key `effect`"),
            (one("drop", "id = \"2133/4\"\nabilities = [{ type = \"kill_boost\", times = 2 }]"), "unknown key `abilities`"),
            (
                one("item", "id = \"100/215\"\neffect = { type = \"recreate\", stats = [\"str\"], at = 2026-11-05 }"),
                "effect: a datetime cannot be a column value",
            ),
        ];
        for (error, needle) in cases {
            assert!(error.contains(needle), "wanted {needle:?}, got {error:?}");
        }
    }
}
