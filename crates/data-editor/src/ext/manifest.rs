//! `extension.toml` (`docs/plans/ext-1-foundation.md` section 5.2): parsed and validated without the database.

use std::collections::BTreeMap;

use serde_json::Value;
use toml::Table;

use super::ids::{self, IdRange};

/// The manifest format this data-editor reads.
pub const FORMAT: i64 = 1;

/// The most script files an extension lists, the largest one and all together (`docs/plans/ext-5-lua.md` section 3.1).
pub const MAX_SCRIPTS: usize = 64;
pub const MAX_SCRIPT_BYTES: usize = 256 * 1024;
pub const MAX_SCRIPTS_TOTAL: usize = 1024 * 1024;

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
    /// The extension's rules (`docs/plans/ext-2-rules.md` section 4.4), by name without the extension's prefix, each as the JSON of its
    /// table; the game server's parser checks them (`ext check`, `ext install`).
    pub rules: BTreeMap<String, Value>,
    /// The Lua API version the scripts are written for; with `scripts` (section 3.1 of the scripts chunk).
    pub api: Option<u32>,
    /// The script files, in load order.
    pub scripts: Vec<String>,
    /// The options, by key, each as the manifest wrote it (validated by the game server's `OptionSpec`).
    pub options: BTreeMap<String, game_server::script::OptionSpec>,
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

/// A script path: `^[a-z0-9_-]+(/[a-z0-9_-]+)*\.lua$`.
pub fn valid_script_path(path: &str) -> bool {
    let Some(stem) = path.strip_suffix(".lua") else { return false };
    !stem.is_empty()
        && stem
            .split('/')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-'))
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
        Some(toml::Value::Array(items)) => {
            items.iter().map(|v| v.as_str().map(str::to_owned).ok_or_else(|| format!("`{key}` must be a list of strings"))).collect()
        }
        Some(other) => Err(format!("`{key}` must be a list, not {}", other.type_str())),
    }
}

fn only_keys(table: &Table, allowed: &[&str], place: &str) -> Result<(), String> {
    for key in table.keys() {
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
        only_keys(
            &table,
            &[
                "format",
                "name",
                "version",
                "title",
                "description",
                "authors",
                "license",
                "ids",
                "settings",
                "rules",
                "api",
                "scripts",
                "options",
                "client",
            ],
            "the manifest",
        )?;
        let name = string(&table, "name", true)?.unwrap_or_default();
        if !valid_name(&name) {
            return Err(format!("name {name:?} must match ^[a-z][a-z0-9-]{{1,39}}$"));
        }
        let version_text = string(&table, "version", true)?.unwrap_or_default();
        let version =
            semver::Version::parse(&version_text).map_err(|e| format!("version {version_text:?} is not a semantic version: {e}"))?;
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

        let mut rules = BTreeMap::new();
        if let Some(value) = table.get("rules") {
            for (key, v) in value.as_table().ok_or("`rules` must be a table of tables, [rules.<name>]")? {
                let rule = v.as_table().ok_or_else(|| format!("[rules.{key}] must be a table"))?;
                rules.insert(key.clone(), serde_json::to_value(rule).map_err(|e| format!("[rules.{key}]: {e}"))?);
            }
        }

        let (api, scripts, options) = scripts_part(&table)?;

        let (mut features, mut art) = (Vec::new(), Vec::new());
        if let Some(value) = table.get("client") {
            let client = value.as_table().ok_or("`client` must be a table")?;
            only_keys(client, &["features", "art"], "[client]")?;
            features = strings(client, "features").map_err(|e| format!("[client] {e}"))?;
            if let Some(toml::Value::Array(list)) = client.get("art") {
                for (n, entry) in list.iter().enumerate() {
                    let entry =
                        entry.as_table().ok_or_else(|| format!("[client] art #{}: must be {{ member = \"…\", file = \"…\" }}", n + 1))?;
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
        Ok(Manifest { name, version, title, description, items, skills, settings, rules, api, scripts, options, features, art, json })
    }
}

/// `api`, `scripts` and `[options.<key>]` (`docs/plans/ext-5-lua.md` section 3.1).
fn scripts_part(table: &Table) -> Result<(Option<u32>, Vec<String>, BTreeMap<String, game_server::script::OptionSpec>), String> {
    let api = match table.get("api") {
        None => None,
        Some(toml::Value::Integer(n)) if *n >= 1 => Some(u32::try_from(*n).map_err(|_| format!("api = {n} is not an API version"))?),
        Some(other) => return Err(format!("`api` must be a whole number from 1, not {other}")),
    };
    let scripts = match table.get("scripts") {
        None => Vec::new(),
        Some(toml::Value::Array(items)) => {
            let mut out: Vec<String> = Vec::new();
            for v in items {
                let Some(path) = v.as_str() else { return Err("`scripts` must be a list of strings".into()) };
                if !valid_script_path(path) {
                    return Err(format!(
                        "scripts: {path:?} is not a script path (^[a-z0-9_-]+(/[a-z0-9_-]+)*\\.lua$, inside the extension's folder)"
                    ));
                }
                if out.iter().any(|p| p == path) {
                    return Err(format!("scripts: {path:?} is listed twice"));
                }
                out.push(path.to_owned());
            }
            if out.is_empty() || out.len() > MAX_SCRIPTS {
                return Err(format!("`scripts` lists 1 to {MAX_SCRIPTS} files (got {})", out.len()));
            }
            out
        }
        Some(other) => return Err(format!("`scripts` must be a list, not {}", other.type_str())),
    };
    let mut options = BTreeMap::new();
    if let Some(value) = table.get("options") {
        let tables = value.as_table().ok_or("`options` must be a table of tables, [options.<key>]")?;
        if tables.len() > game_server::script::options::MAX_OPTIONS {
            return Err(format!("[options]: at most {} options (got {})", game_server::script::options::MAX_OPTIONS, tables.len()));
        }
        for (key, v) in tables {
            if !game_server::script::options::valid_key(key) {
                return Err(format!("[options.{key}]: the key must match ^[a-z][a-z0-9_]{{0,39}}$"));
            }
            let spec = v.as_table().ok_or_else(|| format!("[options.{key}] must be a table"))?;
            let json = serde_json::to_value(spec).map_err(|e| format!("[options.{key}]: {e}"))?;
            options.insert(key.clone(), game_server::script::OptionSpec::parse(&json).map_err(|e| format!("[options.{key}]: {e}"))?);
        }
    }
    match (api, scripts.is_empty()) {
        (Some(_), true) => return Err("`api` without `scripts`: api names the Lua API version the scripts are written for".into()),
        (None, false) => return Err("`scripts` without `api`: say which Lua API version they are written for (api = 1)".into()),
        _ => {}
    }
    if scripts.is_empty() && !options.is_empty() {
        return Err("[options] without `scripts`: options belong to the scripts that read them".into());
    }
    if let Some(api) = api
        && api > game_server::script::API_VERSION
    {
        return Err(format!("needs a newer game server: API {api}, this one has {}", game_server::script::API_VERSION));
    }
    Ok((api, scripts, options))
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

[rules.no-pvp-gain]
type = "pvp_gain"
maps = [35, 36]
exp = 0

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
        assert_eq!(m.rules["no-pvp-gain"], serde_json::json!({ "type": "pvp_gain", "maps": [35, 36], "exp": 0 }));
        assert_eq!(m.rules.len(), 1);
        assert_eq!((m.api, m.scripts.is_empty(), m.options.is_empty()), (None, true, true));
        assert_eq!(m.features, ["a"]);
        assert_eq!(m.art, [Art { member: "Item/Icon/x.dds".into(), file: "art/x.dds".into() }]);
        assert_eq!(m.json["name"], "kill-boost");
    }

    #[test]
    fn a_minimal_manifest_needs_only_the_required_keys() {
        let m = Manifest::parse("format = 1\nname = \"ab\"\nversion = \"0.1.0\"\ntitle = \"t\"\ndescription = \"d\"\n").unwrap();
        assert!(m.items.is_empty() && m.skills.is_empty() && m.settings.is_empty() && m.rules.is_empty());
    }

    #[test]
    fn scripts_api_and_options_parse() {
        let text = r#"
format = 1
name = "kill-streak"
version = "1.0.0"
title = "t"
description = "d"
api = 1
scripts = ["main.lua", "lib/more_2.lua"]

[options.announce_at]
type = "integers"
default = [5, 10]
doc = "Streaks announced."

[options.bounty_gold]
type = "integer"
default = 100000
min = 0
max = 100000000
doc = "Gold."
"#;
        let m = Manifest::parse(text).unwrap();
        assert_eq!((m.api, m.scripts.as_slice()), (Some(1), ["main.lua".to_owned(), "lib/more_2.lua".to_owned()].as_slice()));
        assert_eq!(m.options.keys().collect::<Vec<_>>(), ["announce_at", "bounty_gold"]);
        assert_eq!(m.options["bounty_gold"].max, Some(serde_json::json!(100000000)));
        assert_eq!(m.json["api"], 1, "the manifest keeps api, scripts and options as written");
        assert_eq!(m.json["options"]["announce_at"]["default"], serde_json::json!([5, 10]));
    }

    #[test]
    fn script_manifests_are_refused_by_key() {
        let head = "format = 1\nname = \"ks\"\nversion = \"1.0.0\"\ntitle = \"t\"\ndescription = \"d\"\n";
        let opt = "\n[options.x]\ntype = \"integer\"\ndefault = 1\ndoc = \"d\"\n";
        let cases: Vec<(String, &str)> = vec![
            (format!("{head}scripts = [\"main.lua\"]\n"), "`scripts` without `api`"),
            (format!("{head}api = 1\n"), "`api` without `scripts`"),
            (format!("{head}api = 2\nscripts = [\"main.lua\"]\n"), "needs a newer game server: API 2, this one has 1"),
            (format!("{head}api = 0\nscripts = [\"main.lua\"]\n"), "`api` must be a whole number from 1"),
            (format!("{head}api = \"1\"\nscripts = [\"main.lua\"]\n"), "`api` must be a whole number"),
            (format!("{head}api = 1\nscripts = []\n"), "`scripts` lists 1 to 64 files (got 0)"),
            (format!("{head}api = 1\nscripts = [\"../x.lua\"]\n"), "is not a script path"),
            (format!("{head}api = 1\nscripts = [\"/x.lua\"]\n"), "is not a script path"),
            (format!("{head}api = 1\nscripts = [\"Main.lua\"]\n"), "is not a script path"),
            (format!("{head}api = 1\nscripts = [\"main.txt\"]\n"), "is not a script path"),
            (format!("{head}api = 1\nscripts = [\"a//b.lua\"]\n"), "is not a script path"),
            (format!("{head}api = 1\nscripts = [\"main.lua\", \"main.lua\"]\n"), "listed twice"),
            (format!("{head}api = 1\nscripts = [1]\n"), "list of strings"),
            (
                format!("{head}api = 1\nscripts = [{}]\n", (0..65).map(|i| format!("\"f{i}.lua\"")).collect::<Vec<_>>().join(", ")),
                "1 to 64 files",
            ),
            (format!("{head}{}", opt.trim_start()), "[options] without `scripts`"),
            (
                format!("{head}api = 1\nscripts = [\"m.lua\"]\n[options.X]\ntype = \"integer\"\ndefault = 1\ndoc = \"d\"\n"),
                "[options.X]: the key must match",
            ),
            (
                format!("{head}api = 1\nscripts = [\"m.lua\"]\n[options.x]\ntype = \"integer\"\ndefault = \"a\"\ndoc = \"d\"\n"),
                "[options.x]: default:",
            ),
            (
                format!("{head}api = 1\nscripts = [\"m.lua\"]\n[options.x]\ntype = \"integer\"\ndefault = 1\n"),
                "[options.x]: doc is required",
            ),
            (format!("{head}api = 1\nscripts = [\"m.lua\"]\n[options]\nx = 1\n"), "[options.x] must be a table"),
        ];
        for (text, needle) in cases {
            let error = Manifest::parse(&text).unwrap_err();
            assert!(error.contains(needle), "wanted {needle:?}, got {error:?}");
        }
        // 65 options.
        let many: String = (0..65).map(|i| format!("[options.o{i}]\ntype = \"boolean\"\ndefault = true\ndoc = \"d\"\n")).collect();
        let error = Manifest::parse(&format!("{head}api = 1\nscripts = [\"m.lua\"]\n{many}")).unwrap_err();
        assert!(error.contains("at most 64 options"), "{error}");
        assert!(
            valid_script_path("a.lua") && valid_script_path("a/b-c_d/e1.lua") && !valid_script_path(".lua") && !valid_script_path("a/.lua")
        );
    }

    #[test]
    fn refusals_name_the_key() {
        let no_rules = || GOOD.replace("\n[rules.no-pvp-gain]\ntype = \"pvp_gain\"\nmaps = [35, 36]\nexp = 0\n", "");
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
            (GOOD.replace("[ids]", "options = 1\n[ids]"), "`options` must be a table of tables"),
            (no_rules().replace("[ids]", "rules = 1\n[ids]"), "`rules` must be a table of tables"),
            (no_rules().replace("[ids]", "[rules]\nx = 1\n[ids]"), "[rules.x] must be a table"),
            (GOOD.replace("[ids]", "scripts = 1\n[ids]"), "`scripts` must be a list"),
            (GOOD.replace("[ids]", "api = 1\n[ids]"), "`api` without `scripts`"),
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
