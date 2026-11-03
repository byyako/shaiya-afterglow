//! `extension.toml` (`docs/plans/ext-1-foundation.md` section 5.2): parsed and validated without the database.

use std::collections::BTreeMap;

use serde_json::Value;
use toml::Table;

use super::ids::{self, IdRange};

/// The manifest format this data-editor reads.
pub const FORMAT: i64 = 1;

/// Keys that belong to later chunks: refused, so that an extension cannot rely on them being ignored.
const RESERVED: &[&str] = &["options", "rules", "scripts", "api"];

#[derive(Clone, Debug, PartialEq)]
pub struct Art {
    pub member: String,
    pub file: String,
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub name: String,
    pub version: semver::Version,
    pub title: String,
    pub description: String,
    pub items: Vec<IdRange>,
    pub skills: Vec<IdRange>,
    /// Defaults for game settings, as the text the server's parsers read.
    pub settings: BTreeMap<String, String>,
    pub features: Vec<String>,
    pub art: Vec<Art>,
    /// The manifest as read, for `content.extensions.manifest`.
    pub json: Value,
}

impl Manifest {
    /// The schema an extension's own tables live in: `ext_` and the name with dashes as underscores (the prefix is a parameter for tests).
    pub fn schema(&self, prefix: &str) -> String {
        format!("{prefix}{}", self.name.replace('-', "_"))
    }
}

pub fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (2..=40).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

fn reserved(key: &str) -> String {
    format!("`{key}` is reserved for a later chunk (docs/plans/ext-1-foundation.md 5.2)")
}

fn string(table: &Table, key: &str, required: bool) -> Result<Option<String>, String> {
    match table.get(key) {
        None if required => Err(format!("the required key `{key}` is missing")),
        None => Ok(None),
        Some(toml::Value::String(s)) => Ok(Some(s.clone())),
        Some(other) => Err(format!("`{key}` must be a string, not {}", other.type_str())),
    }
}

fn strings(table: &Table, key: &str) -> Result<Vec<String>, String> {
    match table.get(key) {
        None => Ok(Vec::new()),
        Some(toml::Value::Array(items)) => items
            .iter()
            .map(|v| v.as_str().map(str::to_owned).ok_or_else(|| format!("`{key}` must be a list of strings")))
            .collect(),
        Some(other) => Err(format!("`{key}` must be a list, not {}", other.type_str())),
    }
}

fn only_keys(table: &Table, allowed: &[&str], place: &str) -> Result<(), String> {
    for key in table.keys() {
        if RESERVED.contains(&key.as_str()) && place == "the manifest" {
            return Err(reserved(key));
        }
        if !allowed.contains(&key.as_str()) {
            return Err(format!("unknown key `{key}` in {place}"));
        }
    }
    Ok(())
}

/// A setting's value as the text the server's parsers read: a string as written, a number or boolean as its text, a list joined by `,`.
pub fn setting_text(value: &toml::Value) -> Result<String, String> {
    Ok(match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Array(items) => items
            .iter()
            .map(|v| match v {
                toml::Value::String(s) => Ok(s.clone()),
                toml::Value::Integer(i) => Ok(i.to_string()),
                other => Err(format!("a list holds integers or strings, not {}", other.type_str())),
            })
            .collect::<Result<Vec<_>, _>>()?
            .join(","),
        other => return Err(format!("a {} cannot be a setting", other.type_str())),
    })
}

impl Manifest {
    /// Parses `extension.toml`. Errors name the key, and the caller prefixes the file.
    pub fn parse(text: &str) -> Result<Manifest, String> {
        let table: Table = toml::from_str(text).map_err(|e| format!("not valid TOML: {}", e.to_string().trim_end()))?;
        // The format first: a higher one may have other keys that this reader would call unknown.
        match table.get("format") {
            None => return Err("the required key `format` is missing".into()),
            Some(toml::Value::Integer(f)) if *f == FORMAT => {}
            Some(toml::Value::Integer(f)) if *f > FORMAT => {
                return Err(format!("format = {f}: this data-editor reads format {FORMAT}; it needs a newer data-editor"));
            }
            Some(other) => return Err(format!("`format` must be {FORMAT}, not {other}")),
        }
        only_keys(&table, &["format", "name", "version", "title", "description", "authors", "license", "ids", "settings", "client"], "the manifest")?;
        let name = string(&table, "name", true)?.unwrap_or_default();
        if !valid_name(&name) {
            return Err(format!("name {name:?} must match ^[a-z][a-z0-9-]{{1,39}}$"));
        }
        let version_text = string(&table, "version", true)?.unwrap_or_default();
        let version = semver::Version::parse(&version_text).map_err(|e| format!("version {version_text:?} is not a semantic version: {e}"))?;
        let title = string(&table, "title", true)?.unwrap_or_default();
        let description = string(&table, "description", true)?.unwrap_or_default();
        let _ = strings(&table, "authors")?;
        let _ = string(&table, "license", false)?;

        let (mut items, mut skills) = (Vec::new(), Vec::new());
        if let Some(ids_value) = table.get("ids") {
            let ids_table = ids_value.as_table().ok_or("`ids` must be a table")?;
            only_keys(ids_table, &["items", "skills"], "[ids]")?;
            for text in strings(ids_table, "items")? {
                items.push(ids::parse_range(&text, false).map_err(|e| format!("[ids] items: {e}"))?);
            }
            for text in strings(ids_table, "skills")? {
                skills.push(ids::parse_range(&text, true).map_err(|e| format!("[ids] skills: {e}"))?);
            }
            check_own_overlaps(&items, "items")?;
            check_own_overlaps(&skills, "skills")?;
        }

        let mut settings = BTreeMap::new();
        if let Some(value) = table.get("settings") {
            for (key, v) in value.as_table().ok_or("`settings` must be a table")? {
                settings.insert(key.clone(), setting_text(v).map_err(|e| format!("[settings] {key}: {e}"))?);
            }
        }

        let (mut features, mut art) = (Vec::new(), Vec::new());
        if let Some(value) = table.get("client") {
            let client = value.as_table().ok_or("`client` must be a table")?;
            only_keys(client, &["features", "art"], "[client]")?;
            features = strings(client, "features").map_err(|e| format!("[client] {e}"))?;
            if let Some(toml::Value::Array(list)) = client.get("art") {
                for (n, entry) in list.iter().enumerate() {
                    let entry = entry.as_table().ok_or_else(|| format!("[client] art #{}: must be {{ member = \"…\", file = \"…\" }}", n + 1))?;
                    only_keys(entry, &["member", "file"], &format!("[client] art #{}", n + 1))?;
                    let member = string(entry, "member", true).map_err(|e| format!("[client] art #{}: {e}", n + 1))?.unwrap_or_default();
                    let file = string(entry, "file", true).map_err(|e| format!("[client] art #{}: {e}", n + 1))?.unwrap_or_default();
                    if file.starts_with('/') || file.split('/').any(|p| p == "..") {
                        return Err(format!("[client] art #{}: file {file:?} must stay inside the extension's folder", n + 1));
                    }
                    art.push(Art { member, file });
                }
            } else if client.get("art").is_some() {
                return Err("[client] art must be a list".into());
            }
        }
        let json = serde_json::to_value(&table).map_err(|e| e.to_string())?;
        Ok(Manifest { name, version, title, description, items, skills, settings, features, art, json })
    }
}

fn check_own_overlaps(ranges: &[IdRange], what: &str) -> Result<(), String> {
    for (i, a) in ranges.iter().enumerate() {
        if let Some(b) = ranges[i + 1..].iter().find(|b| a.overlaps(b)) {
            return Err(format!("[ids] {what}: {} and {} overlap", a.text(), b.text()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
format = 1
name = "kill-boost"
version = "1.0.0"
title = "Kill Boost Stones"
description = "Three stones."
authors = ["Shaiya Afterglow"]
license = "MIT"

[ids]
items = ["100/212-214"]
skills = ["200/1-3"]

[settings]
kill_boost_skills = "200/1:2,200/2:5,200/3:10"
guild_party_size = 2
market_maps = [35, 36]

[client]
features = ["a"]
art = [{ member = "Item/Icon/x.dds", file = "art/x.dds" }]
"#;

    #[test]
    fn a_good_manifest_parses() {
        let m = Manifest::parse(GOOD).unwrap();
        assert_eq!((m.name.as_str(), m.version.to_string().as_str(), m.schema("ext_").as_str()), ("kill-boost", "1.0.0", "ext_kill_boost"));
        assert_eq!(m.items, [IdRange { major: 100, low: 212, high: 214 }]);
        assert_eq!(m.skills, [IdRange { major: 200, low: 1, high: 3 }]);
        assert_eq!(m.settings["kill_boost_skills"], "200/1:2,200/2:5,200/3:10");
        assert_eq!(m.settings["guild_party_size"], "2");
        assert_eq!(m.settings["market_maps"], "35,36", "a list is joined like the old variable's text");
        assert_eq!(m.features, ["a"]);
        assert_eq!(m.art, [Art { member: "Item/Icon/x.dds".into(), file: "art/x.dds".into() }]);
        assert_eq!(m.json["name"], "kill-boost");
    }

    #[test]
    fn a_minimal_manifest_needs_only_the_required_keys() {
        let m = Manifest::parse("format = 1\nname = \"ab\"\nversion = \"0.1.0\"\ntitle = \"t\"\ndescription = \"d\"\n").unwrap();
        assert!(m.items.is_empty() && m.skills.is_empty() && m.settings.is_empty());
    }

    #[test]
    fn refusals_name_the_key() {
        let without = |line: &str| GOOD.lines().filter(|l| !l.starts_with(line)).collect::<Vec<_>>().join("\n");
        let cases: Vec<(String, &str)> = vec![
            (GOOD.replace("format = 1", "format = 2"), "format = 2: this data-editor reads format 1; it needs a newer data-editor"),
            (without("format"), "the required key `format` is missing"),
            (without("name"), "the required key `name` is missing"),
            (without("version"), "the required key `version` is missing"),
            (without("title"), "the required key `title` is missing"),
            (without("description"), "the required key `description` is missing"),
            (GOOD.replace("kill-boost", "Kill Boost"), "must match ^[a-z][a-z0-9-]{1,39}$"),
            (GOOD.replace("kill-boost", "k"), "must match"),
            (GOOD.replace("1.0.0", "1.0"), "not a semantic version"),
            (format!("{GOOD}\nextra = 1\n").replace("[ids]", "extra = 1\n[ids]"), "unknown key `extra` in the manifest"),
            (GOOD.replace("[ids]", "options = 1\n[ids]"), "`options` is reserved for a later chunk"),
            (GOOD.replace("[ids]", "rules = 1\n[ids]"), "`rules` is reserved for a later chunk"),
            (GOOD.replace("[ids]", "scripts = 1\n[ids]"), "`scripts` is reserved for a later chunk"),
            (GOOD.replace("[ids]", "api = 1\n[ids]"), "`api` is reserved for a later chunk"),
            (GOOD.replace("items = [", "monsters = [\"1\"]\nitems = ["), "unknown key `monsters` in [ids]"),
            (GOOD.replace("100/212-214", "101/5"), "[ids] items: item type 101"),
            (GOOD.replace("200/1-3", "327"), "[ids] skills: skill 327"),
            (GOOD.replace("[\"100/212-214\"]", "[\"100/212-214\", \"100/214-215\"]"), "overlap"),
            (GOOD.replace("features = [\"a\"]", "features = [\"a\"]\nfonts = []"), "unknown key `fonts` in [client]"),
            (GOOD.replace("art/x.dds", "../x.dds"), "must stay inside the extension's folder"),
            (GOOD.replace("kill-boost\"", "kill-boost\"\nversion = \"2\""), "not valid TOML"),
        ];
        for (text, needle) in cases {
            let error = Manifest::parse(&text).unwrap_err();
            assert!(error.contains(needle), "wanted {needle:?}, got {error:?}");
        }
    }
}
