//! Scripted extensions (`docs/plans/ext-5-lua.md` sections 3.2 to 3.4) against a real PostgreSQL inside throwaway schemas: install with
//! scripts and options, the dry run, `ext scripts`, `remove`, and the editor's `ext.<name>.<key>` option values. Skipped without
//! DATABASE_URL.

mod common;

use std::path::{Path, PathBuf};

use common::*;
use serde_json::json;

/// A temporary extension folder, removed afterwards.
struct Folder(PathBuf);

const OPTIONS: &str = r#"
api = 1
scripts = ["main.lua", "lib/more.lua"]

[options.text]
type = "string"
default = "Hello {name}"
doc = "The welcome text."

[options.gold]
type = "integer"
default = 100
min = 0
max = 1000
doc = "Gold."

[options.maps]
type = "maps"
default = [35]
doc = "Maps."
"#;

const MAIN: &str = "game.on('login', function(p) end)\ngame.command('hi', function(p, args) end, { help = 'says hi' })\n";
const MORE: &str = "game.on('kill', function(p, mob, near) end)\ngame.every(60, function() end)\n";

impl Folder {
    fn new(name: &str, version: &str, extra: &str) -> Self {
        let to = std::env::temp_dir().join(format!("scripts-test-{:08x}", rand::random::<u32>()));
        std::fs::create_dir_all(to.join("lib")).unwrap();
        std::fs::write(to.join("extension.toml"), format!("format = 1\nname = \"{name}\"\nversion = \"{version}\"\ntitle = \"T\"\ndescription = \"D\"\n{extra}\n")).unwrap();
        std::fs::write(to.join("rows.toml"), "").unwrap();
        Self(to)
    }

    fn standard() -> Self {
        Self::new("sx", "1.0.0", OPTIONS).file("main.lua", MAIN).file("lib/more.lua", MORE)
    }

    fn file(self, rel: &str, text: &str) -> Self {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
        self
    }

    fn edit(self, rel: &str, from: &str, to: &str) -> Self {
        let path = self.0.join(rel);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{rel} has no {from:?}");
        std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
        self
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn seeded() -> Option<Db> {
    let db = Db::create().await?;
    db.seed_kill_boost().await;
    db.freeze("both").await;
    Some(db)
}

fn has(lines: &[String], needle: &str) -> bool {
    lines.iter().any(|l| l.contains(needle))
}

fn failure(error: anyhow::Error) -> String {
    match error.downcast::<data_editor::store::Refused>() {
        Ok(r) => r.0,
        Err(e) => format!("{e:#}"),
    }
}

async fn stored(db: &Db, table: &str, extension: &str) -> i64 {
    db.count(format!("SELECT count(*) FROM {}.{table} WHERE extension = '{extension}'", db.s.content)).await
}

#[tokio::test]
async fn a_scripted_extension_installs_with_its_scripts_in_order_and_shows_the_dry_run() {
    let Some(db) = seeded().await else { return };
    let f = Folder::standard();
    let check = db.editor.ext_check(f.path()).await.unwrap();
    assert!(has(&check, "scripts (API 1): 2 files, 3 options") && has(&check, "events: kill, login") && has(&check, "commands: !hi") && has(&check, "timers: 1"), "{check:#?}");
    assert_eq!(stored(&db, "ext_scripts", "sx").await, 0, "a check writes nothing");

    let lines = db.editor.ext_install(f.path(), false).await.unwrap();
    assert!(has(&lines, "installing sx 1.0.0") && has(&lines, "`console <server id> reload` puts its scripts in force"), "{lines:#?}");
    let rows = db.scalar(format!("SELECT jsonb_agg(jsonb_build_object('seq', seq, 'path', path, 'sha', sha256, 'len', length(source)) ORDER BY seq) AS v FROM {}.ext_scripts WHERE extension = 'sx'", db.s.content)).await;
    let rows = rows["v"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0]["seq"].clone(), rows[0]["path"].clone(), rows[1]["seq"].clone(), rows[1]["path"].clone()), (json!(0), json!("main.lua"), json!(1), json!("lib/more.lua")));
    assert_eq!(rows[0]["sha"], json!(data_editor::ext::ops::hex(MAIN.as_bytes())));
    assert_eq!(rows[1]["len"], json!(MORE.len()));
    let options = db.scalar(format!("SELECT jsonb_object_agg(name, spec) AS v FROM {}.ext_options WHERE extension = 'sx'", db.s.content)).await;
    assert_eq!(options["v"]["gold"], json!({"type": "integer", "default": 100, "doc": "Gold.", "min": 0, "max": 1000}));
    assert_eq!(options["v"]["maps"]["default"], json!([35]));
    let manifest = db.value_text(&format!("{}.extensions", db.s.content), "manifest", "name = 'sx'").await;
    assert!(manifest.contains("\"api\": 1") && manifest.contains("\"scripts\"") && manifest.contains("\"options\""), "the manifest keeps the three keys: {manifest}");

    let shown = db.editor.ext_show("sx").await.unwrap();
    assert!(has(&shown, "script main.lua:") && has(&shown, "script lib/more.lua:") && has(&shown, "sha256 "), "{shown:#?}");
    assert!(has(&shown, "option gold (integer) default 100") && has(&shown, "option maps (maps)"), "{shown:#?}");
    assert!(has(&shown, "dry run of the load registers:") && has(&shown, "events: kill, login") && has(&shown, "commands: !hi"), "{shown:#?}");
    let list = db.editor.ext_list().await.unwrap();
    assert!(list[0].contains("scripts") && list[1].starts_with("sx") && list[1].trim_end().ends_with('2'), "{list:#?}");
    let off = db.editor.ext_set_enabled("sx", false).await.unwrap();
    assert!(has(&off, "takes out of force") && has(&off, "its scripts"), "{off:#?}");
    assert_eq!(stored(&db, "ext_scripts", "sx").await, 2, "disable keeps the scripts");
    db.drop().await;
}

#[tokio::test]
async fn a_script_that_does_not_compile_or_load_refuses_check_and_install_naming_file_and_line() {
    let Some(db) = seeded().await else { return };
    let broken = Folder::standard().file("lib/more.lua", "local a = 1\nlocal = = 2\n");
    for error in [db.editor.ext_check(broken.path()).await.unwrap_err(), db.editor.ext_install(broken.path(), false).await.unwrap_err()] {
        let text = failure(error);
        assert!(text.contains("sx/lib/more.lua:2"), "{text}");
    }
    let top = Folder::standard().file("main.lua", "local x = 1\nerror('nope')\n");
    let text = failure(db.editor.ext_install(top.path(), false).await.unwrap_err());
    assert!(text.contains("sx/main.lua:2") && text.contains("nope"), "{text}");
    let missing = Folder::standard();
    std::fs::remove_file(missing.path().join("lib/more.lua")).unwrap();
    assert!(failure(db.editor.ext_check(missing.path()).await.unwrap_err()).contains("lib/more.lua"));
    let big = Folder::standard().file("main.lua", &format!("-- {}\n", "x".repeat(300 * 1024)));
    assert!(failure(db.editor.ext_install(big.path(), false).await.unwrap_err()).contains("at most 256 KiB"));
    let latin = Folder::standard();
    std::fs::write(latin.path().join("main.lua"), [0x2d, 0x2d, 0x20, 0xe9, 0x0a]).unwrap();
    assert!(failure(db.editor.ext_install(latin.path(), false).await.unwrap_err()).contains("not UTF-8"));
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.extensions", db.s.content)).await, 0, "nothing was installed");
    db.drop().await;
}

#[tokio::test]
async fn an_api_above_the_servers_is_refused() {
    let Some(db) = seeded().await else { return };
    let f = Folder::standard().edit("extension.toml", "api = 1", "api = 2");
    let text = failure(db.editor.ext_install(f.path(), false).await.unwrap_err());
    assert!(text.contains("needs a newer game server: API 2, this one has 1"), "{text}");
    let f = Folder::new("sx", "1.0.0", "scripts = [\"main.lua\"]").file("main.lua", MAIN);
    assert!(failure(db.editor.ext_check(f.path()).await.unwrap_err()).contains("`scripts` without `api`"));
    db.drop().await;
}

#[tokio::test]
async fn ext_scripts_replaces_the_scripts_and_refuses_everything_else() {
    let Some(db) = seeded().await else { return };
    let f = Folder::standard();
    db.editor.ext_install(f.path(), false).await.unwrap();
    let before_upgraded: String = db.value_text(&format!("{}.extensions", db.s.content), "coalesce(upgraded_at::text, '-')", "name = 'sx'").await;

    // New scripts and a new option, same version: replaced.
    let g = Folder::standard()
        .file("main.lua", "game.on('login', function(p) end)\ngame.on('logout', function(p) end)\n")
        .edit("extension.toml", "[options.gold]", "[options.extra]\ntype = \"boolean\"\ndefault = true\ndoc = \"E.\"\n\n[options.gold]");
    let lines = db.editor.ext_scripts(g.path()).await.unwrap();
    assert!(has(&lines, "sx 1.0.0: scripts replaced") && has(&lines, "events: kill, login, logout") && has(&lines, "`console <server id> reload` puts them in force"), "{lines:#?}");
    let source = db.value_text(&format!("{}.ext_scripts", db.s.content), "source", "extension = 'sx' AND path = 'main.lua'").await;
    assert!(source.contains("logout"));
    assert_eq!(stored(&db, "ext_options", "sx").await, 4);
    assert_eq!(db.value_text(&format!("{}.extensions", db.s.content), "version", "name = 'sx'").await, "1.0.0");
    assert_eq!(db.value_text(&format!("{}.extensions", db.s.content), "coalesce(upgraded_at::text, '-')", "name = 'sx'").await, before_upgraded, "no upgrade happened");
    assert!(db.value_text(&format!("{}.extensions", db.s.content), "manifest", "name = 'sx'").await.contains("\"extra\""));
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.ext_log WHERE extension = 'sx' AND action = 'scripts'", db.s.content)).await, 1);

    // A different version, a changed title, a setting, a rules file entry, rows: refused, naming the first difference.
    let v2 = Folder::standard().edit("extension.toml", "version = \"1.0.0\"", "version = \"1.0.1\"");
    assert!(failure(db.editor.ext_scripts(v2.path()).await.unwrap_err()).contains("use `ext upgrade`"));
    let title = Folder::standard().edit("extension.toml", "title = \"T\"", "title = \"U\"");
    let text = failure(db.editor.ext_scripts(title.path()).await.unwrap_err());
    assert!(text.contains("rows, rules, settings or migrations changed: use `ext upgrade` with a higher version") && text.contains("manifest"), "{text}");
    let setting = Folder::standard().edit("extension.toml", "[options.text]", "[settings]\nguild_party_size = 2\n\n[options.text]");
    let text = failure(db.editor.ext_scripts(setting.path()).await.unwrap_err());
    assert!(text.contains("changed: use `ext upgrade`") && text.contains("settings"), "{text}");
    let rule = Folder::standard().edit("extension.toml", "[options.text]", "[rules.no-pvp]\ntype = \"pvp_gain\"\nexp = 0\n\n[options.text]");
    assert!(failure(db.editor.ext_scripts(rule.path()).await.unwrap_err()).contains("rules"));
    let rows = Folder::standard().edit("extension.toml", "[options.text]", "[ids]\nskills = [\"200/1\"]\n\n[options.text]").file("rows.toml", "[[skill]]\nid = \"200/1\"\ncopy = { from = \"250/1\" }\n");
    assert!(failure(db.editor.ext_scripts(rows.path()).await.unwrap_err()).contains("changed: use `ext upgrade`"));
    let migration = Folder::standard().file("migrations/0001_t.sql", "CREATE TABLE t (id integer);");
    assert!(failure(db.editor.ext_scripts(migration.path()).await.unwrap_err()).contains("migrations"));
    let unknown = Folder::new("other", "1.0.0", "").file("rows.toml", "");
    assert!(failure(db.editor.ext_scripts(unknown.path()).await.unwrap_err()).contains("is not installed"));
    // None of the refusals changed anything.
    assert_eq!(stored(&db, "ext_options", "sx").await, 4);
    assert_eq!(stored(&db, "ext_settings", "sx").await, 0);
    assert_eq!(stored(&db, "ext_rules", "sx").await, 0);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.ext_migrations WHERE extension = 'sx'", db.s.content)).await, 0);

    // A broken edit changes nothing either.
    let broken = Folder::standard().file("main.lua", "game.on('login', function(p) end\n");
    let text = failure(db.editor.ext_scripts(broken.path()).await.unwrap_err());
    assert!(text.contains("sx/main.lua:"), "{text}");
    let source = db.value_text(&format!("{}.ext_scripts", db.s.content), "source", "extension = 'sx' AND path = 'main.lua'").await;
    assert!(source.contains("logout"));
    db.drop().await;
}

#[tokio::test]
async fn remove_drops_scripts_options_and_stored_values_and_disable_keeps_them() {
    let Some(db) = seeded().await else { return };
    db.editor.ext_install(Folder::standard().path(), false).await.unwrap();
    let c = &db.s.content;
    sqlx::query(sqlx::AssertSqlSafe(format!("INSERT INTO {c}.script_values (extension, character_id, key, value) VALUES ('sx', 7, 'streak', '3')"))).execute(&db.pool).await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("INSERT INTO {c}.script_globals (extension, key, value) VALUES ('sx', 'bounty.7', '5000')"))).execute(&db.pool).await.unwrap();
    db.editor.ext_set_enabled("sx", false).await.unwrap();
    assert_eq!((stored(&db, "script_values", "sx").await, stored(&db, "script_globals", "sx").await), (1, 1), "disable keeps the values");
    let lines = db.editor.ext_remove("sx", false, |_| Ok(true)).await.unwrap();
    assert!(has(&lines, "takes its scripts out of force"), "{lines:#?}");
    for table in ["ext_scripts", "ext_options", "script_values", "script_globals"] {
        assert_eq!(stored(&db, table, "sx").await, 0, "{table}");
    }
    db.drop().await;
}

#[tokio::test]
async fn the_editor_sets_an_extensions_option_checked_against_its_spec() {
    let Some(db) = seeded().await else { return };
    db.editor.ext_install(Folder::standard().path(), false).await.unwrap();
    let cases = [
        (set("setting", "ext.sx.gold", "value", json!("lots")), "not a"),
        (set("setting", "ext.sx.gold", "value", json!(-5)), "is below 0"),
        (set("setting", "ext.sx.gold", "value", json!(5000)), "is above 1000"),
        (set("setting", "ext.sx.gold", "value", json!(1.5)), "whole number"),
        (set("setting", "ext.sx.text", "value", json!(5)), "not a string"),
        (set("setting", "ext.sx.maps", "value", json!(["x"])), "map range"),
        (set("setting", "ext.sx.nothing", "value", json!(1)), "has no option \"nothing\" (its options: gold, maps, text)"),
        (set("setting", "ext.sy.gold", "value", json!(1)), "the extension sy is not installed"),
        (set("setting", "ext.sx.gold", "other", json!(1)), "one field, value"),
    ];
    for (line, expect) in cases {
        let error = refusal(db.editor.draft_add(&line, "tester").await.unwrap_err());
        assert!(error.contains(expect), "{line:?}: {error}");
    }
    assert!(db.editor.draft_list().await.unwrap().is_empty());

    db.draft(set("setting", "ext.sx.gold", "value", json!(250))).await;
    db.draft(set("setting", "ext.sx.text", "value", json!("Hi there, {name}"))).await;
    db.draft(add("setting", "ext.sx.maps", json!({"value": [35, "40-42"]}))).await;
    let applied = db.apply("tune sx").await;
    assert_eq!(applied.effects.iter().map(|e| e.name()).collect::<Vec<_>>(), ["live"], "an option takes effect at reload");
    let values = db.scalar(format!("SELECT jsonb_object_agg(name, value) AS v FROM {}.setting_values", db.s.content)).await;
    assert_eq!(values["v"], json!({"ext.sx.gold": "250", "ext.sx.text": "\"Hi there, {name}\"", "ext.sx.maps": "[35,\"40-42\"]"}), "JSON text");
    let shown = db.editor.ext_show("sx").await.unwrap();
    assert!(has(&shown, "option gold (integer) default 100, data editor 250"), "{shown:#?}");

    // The same value again changes nothing; a number given as text is read as JSON; an unset puts the original text back.
    db.draft(set("setting", "ext.sx.gold", "value", json!("300"))).await;
    let second = db.apply("again").await;
    assert_eq!(db.value_text(&format!("{}.setting_values", db.s.content), "value", "name = 'ext.sx.gold'").await, "300");
    db.editor.undo(second.change_set, "tester").await.unwrap();
    assert_eq!(db.value_text(&format!("{}.setting_values", db.s.content), "value", "name = 'ext.sx.gold'").await, "250");
    db.editor.undo(applied.change_set, "tester").await.unwrap();
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.setting_values", db.s.content)).await, 0);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.fields", db.s.edits)).await, 0);

    // `apply` writes an override again from the edits, and one the extension no longer accepts is reported and left alone.
    db.draft(set("setting", "ext.sx.text", "value", json!("plain"))).await;
    db.apply("text").await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {}.setting_values", db.s.content))).execute(&db.pool).await.unwrap();
    let report = db.editor.apply_edits().await.unwrap();
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert_eq!(db.value_text(&format!("{}.setting_values", db.s.content), "value", "name = 'ext.sx.text'").await, "\"plain\"");
    sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {}.setting_values", db.s.content))).execute(&db.pool).await.unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {}.ext_options WHERE name = 'text'", db.s.content))).execute(&db.pool).await.unwrap();
    let report = db.editor.apply_edits().await.unwrap();
    assert!(report.problems.iter().any(|p| p.key == "ext.sx.text" && p.text.contains("has no option")), "{:?}", report.problems);
    db.drop().await;
}

/// `ext check`'s work that needs no database, on every folder of `extensions/`: the manifest, the rows and the dry run of the scripts.
#[test]
fn every_bundled_extension_passes_check() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let mut scripted = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let dir = entry.unwrap().path();
        if !dir.is_dir() {
            continue;
        }
        let plan = data_editor::ext::ops::read_folder(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        assert!(dir.join("README.md").is_file(), "{} has a README", dir.display());
        if let Some(summary) = &plan.summary {
            assert!(summary.handlers() > 0, "{} registers something", plan.manifest.name);
            scripted.push(plan.manifest.name.clone());
        }
    }
    scripted.sort();
    assert_eq!(scripted, ["boss-rewards", "kill-streak", "monster-madness", "welcome"]);
}
