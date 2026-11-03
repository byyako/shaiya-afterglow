//! `data-editor ext …` against a real PostgreSQL inside throwaway schemas (see `common`): install, upgrade, remove, enable, disable and
//! their refusals, with a Kill-Boost-like fixture extension (tests/fixtures/ext/kb). Skipped (and says so) when DATABASE_URL is not set.

mod common;

use std::path::{Path, PathBuf};

use common::*;
use data_editor::registry::Side;
use serde_json::{Value, json};

// ------------------------------------------------------------------------------------------------------------------- the folders

/// A copy of the fixture extension (or a bare one) in a temporary folder, edited by the test; removed afterwards.
struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ext/kb");
        let to = std::env::temp_dir().join(format!("ext-test-{:08x}", rand::random::<u32>()));
        std::fs::create_dir_all(&to).unwrap();
        for file in ["extension.toml", "rows.toml"] {
            std::fs::copy(from.join(file), to.join(file)).unwrap();
        }
        Self(to)
    }

    /// An extension of its own: a manifest with the name, version and `extra` lines (keys, tables), and the rows file.
    fn bare(name: &str, version: &str, extra: &str, rows: &str) -> Self {
        let to = std::env::temp_dir().join(format!("ext-test-{:08x}", rand::random::<u32>()));
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(to.join("extension.toml"), format!("format = 1\nname = \"{name}\"\nversion = \"{version}\"\ntitle = \"T\"\ndescription = \"D\"\n{extra}\n")).unwrap();
        std::fs::write(to.join("rows.toml"), rows).unwrap();
        Self(to)
    }

    fn edit(self, file: &str, from: &str, to: &str) -> Self {
        let path = self.0.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{file} has no {from:?}");
        std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
        self
    }

    fn append(self, file: &str, text: &str) -> Self {
        let path = self.0.join(file);
        let old = std::fs::read_to_string(&path).unwrap_or_default();
        std::fs::write(&path, format!("{old}\n{text}\n")).unwrap();
        self
    }

    fn write(self, rel: &str, text: &str) -> Self {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
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

async fn seeded(full: bool) -> Option<Db> {
    let db = if full { Db::create_full().await } else { Db::create().await }?;
    db.seed_kill_boost().await;
    db.freeze("both").await;
    Some(db)
}

async fn hashes(db: &Db) -> Vec<String> {
    db.editor.content_hashes(&[db.s.defs.clone(), db.s.world.clone()]).await.unwrap()
}

async fn install(db: &Db, f: &Folder) -> Vec<String> {
    db.editor.ext_install(f.path(), false).await.unwrap()
}

async fn since(db: &Db, table: &str, filter: &str) -> Value {
    db.scalar(format!("SELECT since_patch AS v FROM {}.{table} WHERE {filter}", db.s.content)).await["v"].clone()
}

async fn count_in_content(db: &Db, table: &str) -> i64 {
    db.count(format!("SELECT count(*) FROM {}.{table}", db.s.content)).await
}

async fn item(db: &Db, itemid: i64) -> Value {
    db.scalar(format!("SELECT to_jsonb(t) AS v FROM {}.items t WHERE itemid = {itemid}", db.s.defs)).await["v"].clone()
}

async fn skill(db: &Db, id: i64, level: i64) -> Value {
    db.scalar(format!("SELECT to_jsonb(t) AS v FROM {}.skills t WHERE skillid = {id} AND skilllevel = {level}", db.s.defs)).await["v"].clone()
}

fn has(lines: &[String], needle: &str) -> bool {
    lines.iter().any(|l| l.contains(needle))
}

// --------------------------------------------------------------------------------------------------------------------- install

#[tokio::test]
async fn install_writes_the_computed_rows_and_the_records_and_a_rebuild_agrees() {
    let Some(db) = seeded(false).await else { return };
    let before = hashes(&db).await;
    let f = Folder::new();
    let lines = install(&db, &f).await;
    assert!(has(&lines, "installing kb 1.0.0") && has(&lines, "6 rows changed, 0 rows added, 6 client rows, 1 settings"), "{lines:#?}");
    assert!(has(&lines, "restart the game servers") && has(&lines, "client_patch.py check"), "{lines:#?}");
    assert_ne!(before, hashes(&db).await);

    // the rows as the entries compute them: the original row, the copied columns, then `set`
    let stone = item(&db, 100212).await;
    assert_eq!((stone["itemname"].clone(), stone["range"].clone(), stone["attacktime"].clone()), (json!("Kill Boost Stone"), json!(200), json!(1)));
    assert_eq!((stone["reqlevel"].clone(), stone["country"].clone(), stone["buy"].clone(), stone["grade"].clone()), (json!(1), json!(2), json!(100), json!(3)));
    assert_eq!((stone["type"].clone(), stone["typeid"].clone(), stone["rowid"].clone()), (json!(100), json!(212), json!(11)), "the key and the row number stay");
    assert_eq!(item(&db, 100214).await["attacktime"], json!(3));
    let buff = skill(&db, 200, 1).await;
    assert_eq!((buff["skillname"].clone(), buff["keeptime"].clone(), buff["fixrange"].clone(), buff["typedetail"].clone()), (json!("Kill Boost"), json!(1), json!(1), json!(82)));
    assert_eq!((buff["abilitytype1"].clone(), buff["abilitytype2"].clone(), buff["abilityvalue2"].clone()), (json!(0), json!(0), json!(0)), "the ability is gone");
    assert_eq!(buff["updatedate"], json!("2001-01-01T00:00:01"), "the placeholder's date stays");
    assert_eq!(skill(&db, 200, 3).await["skillname"], json!("Kill Boost Lv3"));

    // the records
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.ext_rows WHERE extension = 'kb' AND action = 'change'", db.s.content)).await, 6);
    let owned = db.scalar(format!("SELECT row AS v FROM {}.ext_rows WHERE tbl = 'defs.items' AND key = '100212'", db.s.content)).await;
    assert_eq!(owned["v"], stone, "ext_rows holds the whole row as written");
    assert_eq!(count_in_content(&db, "ext_ranges").await, 2);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.ext_settings WHERE name = 'kill_boost_skills' AND value = '200/1:2,200/2:5,200/3:10'", db.s.content)).await, 1);
    let client = db.scalar(format!("SELECT to_jsonb(c) AS v FROM {}.client_items c WHERE type = 100 AND typeid = 212", db.s.content)).await["v"].clone();
    assert_eq!((client["look_type"].clone(), client["look_typeid"].clone(), client["keep"].clone(), client["since_patch"].clone()), (json!(100), json!(108), json!(false), Value::Null));
    assert_eq!((client["note"].clone(), client["model"].clone()), (json!("kb 100/212"), Value::Null), "the look-alike is kept as a reference, not resolved");
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.client_skills WHERE note = 'kb buff level 1'", db.s.content)).await, 1);
    assert_eq!(count_in_content(&db, "client_skills").await, 3);
    assert_eq!(count_in_content(&db, "ext_log").await, 1);

    // again: refused; --skip-installed with the same version: nothing; with another: refused naming `ext upgrade`
    let message = refusal(db.editor.ext_install(f.path(), false).await.unwrap_err());
    assert!(message.contains("kb 1.0.0 is installed already") && message.contains("ext upgrade"), "{message}");
    let again = db.editor.ext_install(f.path(), true).await.unwrap();
    assert_eq!(again, ["kb 1.0.0 is already installed; nothing to do"]);
    let other = Folder::new().edit("extension.toml", "version = \"1.0.0\"", "version = \"1.1.0\"");
    let message = refusal(db.editor.ext_install(other.path(), true).await.unwrap_err());
    assert!(message.contains("ext upgrade"), "{message}");
    assert_eq!(count_in_content(&db, "ext_log").await, 1, "nothing was written");

    // a rebuild from the originals and the stored rows writes the same bytes (the strongest test: both paths agree)
    let incremental = hashes(&db).await;
    let rebuilt = db.editor.content_rebuild(&[Side::Defs, Side::World]).await.unwrap();
    assert_eq!(rebuilt[0].ext_rows, 6);
    assert_eq!(incremental, hashes(&db).await);
    assert_eq!(db.editor.content_check().await.unwrap(), Vec::<String>::new());
    let list = db.editor.ext_list().await.unwrap();
    assert!(list.iter().any(|l| l.starts_with("kb ") && l.contains("1.0.0") && l.contains("enabled")), "{list:#?}");
    let shown = db.editor.ext_show("kb").await.unwrap();
    assert!(has(&shown, "itemname: \"???177\" -> \"Kill Boost Stone\"") && has(&shown, "ids: item 100/212-214"), "{shown:#?}");
    db.drop().await;
}

#[tokio::test]
async fn an_install_without_imports_is_refused_with_the_setup_hint() {
    let Some(db) = Db::create().await else { return };
    let message = refusal(db.editor.ext_install(Folder::new().path(), false).await.unwrap_err());
    assert!(message.contains("defs has no import record") && message.contains("tools/setup.sh") && message.contains("decision 11"), "{message}");
    db.drop().await;
}

#[tokio::test]
async fn every_refusal_says_why_and_leaves_nothing_behind() {
    let Some(db) = seeded(false).await else { return };
    let before = hashes(&db).await;
    let drop_entry = |set: &str| Folder::new().append("rows.toml", &format!("[[drop]]\nid = \"2133/4\"\nset = {{ {set} }}"));
    let cases: Vec<(&str, Folder, &str)> = vec![
        ("format 2", Folder::new().edit("extension.toml", "format = 1", "format = 2"), "needs a newer data-editor"),
        ("an unknown manifest key", Folder::new().edit("extension.toml", "[ids]", "extra = 1\n[ids]"), "unknown key `extra`"),
        ("options", Folder::new().edit("extension.toml", "[ids]", "options = 1\n[ids]"), "`options` is reserved for a later chunk"),
        ("rules", Folder::new().edit("extension.toml", "[ids]", "rules = 1\n[ids]"), "`rules` is reserved for a later chunk"),
        ("scripts", Folder::new().edit("extension.toml", "[ids]", "scripts = 1\n[ids]"), "`scripts` is reserved for a later chunk"),
        ("api", Folder::new().edit("extension.toml", "[ids]", "api = 1\n[ids]"), "`api` is reserved for a later chunk"),
        ("type 101", Folder::new().edit("extension.toml", "\"100/212-214\"", "\"101/5\""), "item type 101"),
        ("a typeid past the group's count", Folder::new().edit("extension.toml", "\"100/212-214\"", "\"43/5\""), "holds 1 to 4"),
        ("a group with no records", Folder::new().edit("extension.toml", "\"100/212-214\"", "\"26/1\""), "has no records"),
        ("skill 327", Folder::new().edit("extension.toml", "\"200/1-3\"", "\"327\""), "skills 1 to 326"),
        ("level 100", Folder::new().edit("extension.toml", "\"200/1-3\"", "\"200/100\""), "level 100 is the monsters'"),
        ("an entry outside its ranges", Folder::new().edit("rows.toml", "id = \"100/214\"", "id = \"100/215\""), "lies outside the ids [ids] declares for items (100/212-214)"),
        ("an unknown column", Folder::new().edit("rows.toml", "range = 200, attacktime = 1", "rnage = 200, attacktime = 1"), "column \"rnage\" is not a column of defs.items"),
        ("rowid in set", Folder::new().edit("rows.toml", "itemname = \"Kill Boost Stone\",", "rowid = 5, itemname = \"Kill Boost Stone\","), "part of the key or the row's own number"),
        ("a key column in set", Folder::new().edit("rows.toml", "itemname = \"Kill Boost Stone\",", "itemid = 5, itemname = \"Kill Boost Stone\","), "part of the key or the row's own number"),
        ("range past the client's byte", Folder::new().edit("rows.toml", "range = 200, attacktime = 1", "range = 300, attacktime = 1"), "does not fit the client's byte"),
        (
            "a changed item without client or server_only",
            Folder::new().edit("rows.toml", "client = { name = \"Kill Boost Stone\", look = \"100/108\", description = \"Player kills count double for 1 hour.\" }\n", ""),
            "needs `client = { … }` (or \"keep\")",
        ),
        ("non-ASCII client text", Folder::new().edit("rows.toml", "count double for 1 hour.\" }\n\n[[item]]", "count d\u{e9}ble for 1 hour.\" }\n\n[[item]]"), "not printable ASCII"),
        ("a copy from a row that is not an original", Folder::new().edit("rows.toml", "copy = { from = \"100/108\", columns = [\"reqlevel\"", "copy = { from = \"100/250\", columns = [\"reqlevel\""), "copy from 100/250 is not an original row"),
        ("an unknown setting", Folder::new().edit("extension.toml", "kill_boost_skills = ", "nosuch = \"1\"\nkill_boost_skills = "), "\"nosuch\" is not a game setting"),
        ("a bad setting value", Folder::new().edit("extension.toml", "200/3:10", "200/3:1000"), "times 1000"),
        ("a drop column that is not grade or droprate", drop_entry("itemorder = 5"), "part of the key or the row's own number"),
        ("a drop column of another table", drop_entry("itemname = \"x\""), "is not a column of defs.mob_items"),
        ("a drop chance out of range", drop_entry("droprate = 101"), "droprate of entry 4 must be from 0 to 100"),
        ("a drop entry that is not an original row", Folder::new().append("rows.toml", "[[drop]]\nid = \"9999/4\"\nset = { grade = 3 }"), "may only change grade and droprate of an original drop entry"),
        ("a new row without a copy", Folder::new().append("rows.toml", "[[skill]]\nid = \"201/1\"\nserver_only = \"x\"").edit("extension.toml", "\"200/1-3\"", "\"200/1-3\", \"201\""), "a new row needs `copy"),
    ];
    for (what, folder, needle) in cases {
        let error = db.editor.ext_install(folder.path(), false).await.unwrap_err();
        let message = refusal(error);
        assert!(message.contains(needle), "{what}: wanted {needle:?}, got {message:?}");
        assert_eq!(count_in_content(&db, "extensions").await, 0, "{what}");
        assert_eq!(before, hashes(&db).await, "{what}: a refusal writes nothing");
    }
    // a migration that writes into defs: rolled back, nothing left behind
    let sneaky = Folder::new().write("migrations/0001_oops.sql", &format!("CREATE TABLE notes (id int); UPDATE {}.mobs SET hp = 1;", db.s.defs));
    let message = refusal(db.editor.ext_install(sneaky.path(), false).await.unwrap_err());
    assert!(message.contains("changed rows of defs, world or client"), "{message}");
    assert_eq!(db.count(format!("SELECT count(*) FROM pg_namespace WHERE nspname = '{}kb'", db.s.ext_prefix)).await, 0, "the extension's schema went with the rollback");
    assert_eq!(count_in_content(&db, "ext_migrations").await, 0);
    assert_eq!(before, hashes(&db).await);
    let bad_name = Folder::new().write("migrations/0002_gap.sql", "SELECT 1;");
    let message = refusal(db.editor.ext_install(bad_name.path(), false).await.unwrap_err());
    assert!(message.contains("numbered 0001, 0002"), "{message}");
    db.drop().await;
}

#[tokio::test]
async fn ranges_keys_and_settings_belong_to_one_extension() {
    let Some(db) = seeded(false).await else { return };
    let f = Folder::new();
    install(&db, &f).await;
    // a range that overlaps another extension's, enabled or not
    let overlap = Folder::bare("kb2", "1.0.0", "[ids]\nitems = [\"100/214-215\"]", "");
    let message = refusal(db.editor.ext_install(overlap.path(), false).await.unwrap_err());
    assert!(message.contains("overlap 100/212-214 of the extension kb"), "{message}");
    db.editor.ext_set_enabled("kb", false).await.unwrap();
    let message = refusal(db.editor.ext_install(overlap.path(), false).await.unwrap_err());
    assert!(message.contains("overlap"), "a disabled extension still owns its ids: {message}");
    // a setting another enabled extension sets
    let clash = Folder::bare("kb3", "1.0.0", "[settings]\nkill_boost_skills = \"200/1:3\"", "");
    db.editor.ext_set_enabled("kb", true).await.unwrap();
    let message = refusal(db.editor.ext_install(clash.path(), false).await.unwrap_err());
    assert!(message.contains("kill_boost_skills is set by the enabled extension kb"), "{message}");
    // a disabled extension's settings do not clash: it installs, and kb cannot be enabled again while it counts
    db.editor.ext_set_enabled("kb", false).await.unwrap();
    install(&db, &clash).await;
    let message = refusal(db.editor.ext_set_enabled("kb", true).await.unwrap_err());
    assert!(message.contains("kill_boost_skills is set by the enabled extension kb3"), "{message}");
    // a drop entry is owned per row
    let first = Folder::bare("dropa", "1.0.0", "", "[[drop]]\nid = \"2133/4\"\nset = { grade = 3 }");
    let second = Folder::bare("dropb", "1.0.0", "", "[[drop]]\nid = \"2133/4\"\nset = { grade = 4 }");
    install(&db, &first).await;
    let message = refusal(db.editor.ext_install(second.path(), false).await.unwrap_err());
    assert!(message.contains("defs.mob_items 2133/4 is owned by the extension dropa"), "{message}");
    assert_eq!(db.value("mob_items", "grade", "mobid = 2133 AND itemorder = 4").await, json!(3));
    db.drop().await;
}

#[tokio::test]
async fn with_a_client_schema_the_text_and_the_look_alike_are_checked_against_it() {
    let Some(db) = seeded(false).await else { return };
    let c = &db.s.client;
    run(&db.pool, format!(
        "CREATE SCHEMA {c};
         CREATE TABLE {c}.items (grp smallint, idx smallint, name text, description text);
         CREATE TABLE {c}.skills (skill_id smallint, slot smallint, name text, description text);
         INSERT INTO {c}.items VALUES (100, 108, 'Mini Sun EXP Stone', 'Adds experience'), (100, 109, 'A name of thirty characters.....', '{}');
         INSERT INTO {c}.skills VALUES (250, 1, 'Mini Sun EXP Buff Name', '{}')", "d".repeat(60), "s".repeat(50)
    )).await;
    let long = Folder::new().edit("rows.toml", "count double for 1 hour.\" }\n\n[[item]]", &format!("{}\" }}\n\n[[item]]", "x".repeat(61)));
    let message = refusal(db.editor.ext_install(long.path(), false).await.unwrap_err());
    assert!(message.contains("client description of 74 bytes is longer than the longest item description in the original (60)"), "{message}");
    let missing = Folder::new().edit("rows.toml", "look = \"100/108\", description = \"Player kills count double", "look = \"100/110\", description = \"Player kills count double");
    let message = refusal(db.editor.ext_install(missing.path(), false).await.unwrap_err());
    assert!(message.contains("look 100/110 is not a record of the client's items"), "{message}");
    let long_skill = Folder::new().edit("rows.toml", "description = \"Player kills count 5 times for 1 hour.\"", &format!("description = \"{}\"", "y".repeat(51)));
    let message = refusal(db.editor.ext_install(long_skill.path(), false).await.unwrap_err());
    assert!(message.contains("client description of 51 bytes is longer than the longest skill description"), "{message}");
    assert_eq!(count_in_content(&db, "extensions").await, 0);
    let lines = install(&db, &Folder::new()).await;
    assert!(has(&lines, "6 client rows"), "{lines:#?}");
    db.drop().await;
}

#[tokio::test]
async fn an_edit_on_an_owned_row_survives_install_upgrade_and_a_rebuild() {
    let Some(db) = seeded(true).await else { return };
    // an edit made before the extension: the grade of the placeholder 100212 (0), set to 9
    db.draft(set("item_grade", "100212", "grade", json!(9))).await;
    let applied = db.apply("grade of a placeholder").await;
    assert_eq!(item(&db, 100212).await["grade"], json!(9));
    let lines = install(&db, &Folder::new()).await;
    assert_eq!(item(&db, 100212).await["grade"], json!(9), "the edit stays on top of the extension's row (which copies grade 3)");
    assert!(has(&lines, "PROBLEM item_grade 100212 grade") && has(&lines, "the content under your edit changed grade 0 -> 3"), "{lines:#?}");
    let original = db.scalar(format!("SELECT original AS v FROM {}.fields WHERE target = 'item_grade' AND key = '100212'", db.s.edits)).await;
    assert_eq!(original["v"], json!(3), "its original is what the content below gives now");
    let before = hashes(&db).await;
    let rebuilt = db.editor.content_rebuild(&[Side::Defs]).await.unwrap();
    assert!(rebuilt[0].report.problems.is_empty(), "{:?}", rebuilt[0].report.problems);
    assert_eq!(before, hashes(&db).await);
    // an upgrade keeps it too
    let up = Folder::new().edit("extension.toml", "version = \"1.0.0\"", "version = \"1.0.1\"");
    db.editor.ext_upgrade(up.path()).await.unwrap();
    assert_eq!(item(&db, 100212).await["grade"], json!(9));
    // and it blocks the removal of the row until it is undone
    let message = refusal(db.editor.ext_remove("kb", false, |_| Ok(true)).await.unwrap_err());
    assert!(message.contains(&format!("edit item_grade 100212 grade (change set #{})", applied.change_set)) && message.contains("--force does not clear references"), "{message}");
    // (an Undo would set the value the field had before the edit, 0; the edit's original is 3 now, so the edit is taken off with `unset`)
    db.draft(unset("item_grade", "100212", "grade")).await;
    db.apply("take the edit off").await;
    let lines = db.editor.ext_remove("kb", false, |_| Ok(true)).await.unwrap();
    assert!(has(&lines, "removed kb 1.0.1: 6 rows restored"), "{lines:#?}");
    assert_eq!(item(&db, 100212).await["itemname"], json!("???177"));
    db.drop().await;
}

#[tokio::test]
async fn migrations_live_in_the_extensions_own_schema_and_go_with_it() {
    let Some(db) = seeded(true).await else { return };
    let schema = format!("{}kb", db.s.ext_prefix);
    let f = Folder::new()
        .write("migrations/0001_notes.sql", "CREATE TABLE notes (id int PRIMARY KEY, text text); INSERT INTO notes VALUES (1, 'hello');")
        .write("migrations/0002_more.sql", "INSERT INTO notes VALUES (2, 'again');");
    let lines = install(&db, &f).await;
    assert!(has(&lines, "ran 2 migration(s) in the schema"), "{lines:#?}");
    assert_eq!(db.count(format!("SELECT count(*) FROM {schema}.notes")).await, 2);
    assert_eq!(count_in_content(&db, "ext_migrations").await, 2);
    // an applied migration whose file changed is refused at an upgrade; a new one runs
    let changed = Folder::new()
        .edit("extension.toml", "version = \"1.0.0\"", "version = \"1.1.0\"")
        .write("migrations/0001_notes.sql", "CREATE TABLE notes (id int PRIMARY KEY, text text); INSERT INTO notes VALUES (1, 'HELLO');")
        .write("migrations/0002_more.sql", "INSERT INTO notes VALUES (2, 'again');");
    let message = refusal(db.editor.ext_upgrade(changed.path()).await.unwrap_err());
    assert!(message.contains("migration 0001 was applied as it was before"), "{message}");
    let fine = Folder::new()
        .edit("extension.toml", "version = \"1.0.0\"", "version = \"1.1.0\"")
        .write("migrations/0001_notes.sql", "CREATE TABLE notes (id int PRIMARY KEY, text text); INSERT INTO notes VALUES (1, 'hello');")
        .write("migrations/0002_more.sql", "INSERT INTO notes VALUES (2, 'again');")
        .write("migrations/0003_third.sql", "INSERT INTO notes VALUES (3, 'third');");
    let lines = db.editor.ext_upgrade(fine.path()).await.unwrap();
    assert!(has(&lines, "ran 1 migration(s)"), "{lines:#?}");
    assert_eq!(db.count(format!("SELECT count(*) FROM {schema}.notes")).await, 3);
    let lines = db.editor.ext_remove("kb", false, |_| Ok(true)).await.unwrap();
    assert!(has(&lines, &format!("dropped the schema {schema}")), "{lines:#?}");
    assert_eq!(db.count(format!("SELECT count(*) FROM pg_namespace WHERE nspname = '{schema}'")).await, 0);
    db.drop().await;
}

// ---------------------------------------------------------------------------------------------------------------- remove / upgrade

#[tokio::test]
async fn remove_restores_every_table_and_takes_the_added_rows_out() {
    let Some(db) = seeded(true).await else { return };
    let before = hashes(&db).await;
    let with_add = || {
        Folder::new()
            .edit("extension.toml", "\"100/212-214\"", "\"100/212-215\"")
            .append(
                "rows.toml",
                "[[item]]\nid = \"100/215\"\ncopy = { from = \"100/108\" }\nset = { itemname = \"Extra Stone\" }\nclient = { name = \"Extra Stone\", look = \"100/108\", description = \"An added row.\" }",
            )
    };
    let f = with_add();
    let lines = install(&db, &f).await;
    assert!(has(&lines, "6 rows changed, 1 rows added, 7 client rows"), "{lines:#?}");
    let added = item(&db, 100215).await;
    assert_eq!((added["itemname"].clone(), added["type"].clone(), added["typeid"].clone(), added["rowid"].clone()), (json!("Extra Stone"), json!(100), json!(215), json!(14)));
    assert_eq!(added["buy"], json!(100), "an added row starts as a copy of the source row");
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.ext_rows WHERE action = 'add'", db.s.content)).await, 1);
    assert_eq!(db.editor.content_check().await.unwrap(), Vec::<String>::new());

    let lines = db.editor.ext_remove("kb", false, |_| Ok(true)).await.unwrap();
    assert!(has(&lines, "removed kb 1.0.0: 7 rows restored") && has(&lines, "restart the game servers"), "{lines:#?}");
    assert_eq!(before, hashes(&db).await, "every table is as it was before the install");
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.items WHERE itemid = 100215", db.s.defs)).await, 0, "the added row is gone");
    for table in ["extensions", "ext_rows", "ext_ranges", "ext_settings", "client_items", "client_skills", "server_only"] {
        assert_eq!(count_in_content(&db, table).await, 0, "{table}");
    }
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.ext_log WHERE action IN ('install', 'remove')", db.s.content)).await, 2);
    assert_eq!(db.editor.content_check().await.unwrap(), Vec::<String>::new());
    // removed, it can be installed again
    install(&db, &with_add()).await;
    db.drop().await;
}

async fn make_holder(db: &Db) -> (i32, i32) {
    let account: i32 = sqlx::query_scalar("INSERT INTO accounts (username, password_hash) VALUES ('holder', 'x') RETURNING id").fetch_one(&db.pool).await.unwrap();
    let character: i32 = sqlx::query_scalar("INSERT INTO characters (user_id, server_id, slot, name) VALUES ($1, 1, 0, 'Hero') RETURNING id").bind(account).fetch_one(&db.pool).await.unwrap();
    (account, character)
}

#[tokio::test]
async fn players_who_hold_the_items_stop_the_removal_and_force_deletes_them_after_a_confirmation() {
    let Some(db) = seeded(true).await else { return };
    install(&db, &Folder::new()).await;
    let (account, character) = make_holder(&db).await;
    sqlx::query("INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, count) VALUES ($1, 1, 4, 100212, 100, 212, 3)").bind(character).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO character_quick_slots (character_id, position, bar, slot, bag, number) VALUES ($1, 0, 0, 1, 1, 4), ($1, 1, 0, 2, 1, 9)").bind(character).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO bank_items (user_id, server_id, slot, item_id, type, type_id, count) VALUES ($1, 1, 7, 100213, 100, 213, 1)").bind(account).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO character_skills (character_id, number, skill_id, level) VALUES ($1, 0, 200, 2)").bind(character).execute(&db.pool).await.unwrap();
    sqlx::query("INSERT INTO character_applied_skills (character_id, position, skill_id, level, seconds) VALUES ($1, 0, 200, 1, 3000)").bind(character).execute(&db.pool).await.unwrap();
    let message = refusal(db.editor.ext_remove("kb", false, |_| Ok(true)).await.unwrap_err());
    for needle in [
        "character Hero (",
        "3 × 100/212 in bag 1, slot 4",
        "account holder (",
        "1 × 100/213 in the bank of server 1, slot 7",
        "knows skill 200/2",
        "has the buff 200/1 on, 3000 s left",
        "--force",
    ] {
        assert!(message.contains(needle), "wanted {needle:?} in {message}");
    }
    assert_eq!(count_in_content(&db, "extensions").await, 1, "a refused removal writes nothing");

    // --force: refused while a game server reports players with a fresh heartbeat
    sqlx::query("INSERT INTO game_servers (id, name, address, players, last_seen) VALUES (1, 'T', '10.0.0.1', 3, now())").execute(&db.pool).await.unwrap();
    let message = refusal(db.editor.ext_remove("kb", true, |_| Ok(true)).await.unwrap_err());
    assert!(message.contains("the game servers report 3 players online"), "{message}");
    sqlx::query("UPDATE game_servers SET last_seen = now() - interval '5 minutes'").execute(&db.pool).await.unwrap();
    // a confirmation that is not given deletes nothing
    let message = refusal(db.editor.ext_remove("kb", true, |held| { assert!(held.len() >= 4); Ok(false) }).await.unwrap_err());
    assert!(message.contains("not confirmed"), "{message}");
    assert_eq!(db.count("SELECT count(*) FROM character_items".into()).await, 1);
    let lines = db.editor.ext_remove("kb", true, |_| Ok(true)).await.unwrap();
    assert!(has(&lines, "deleted 4 held rows (logged)") && has(&lines, "removed kb 1.0.0"), "{lines:#?}");
    for table in ["character_items", "bank_items", "character_skills", "character_applied_skills"] {
        assert_eq!(db.count(format!("SELECT count(*) FROM {table}")).await, 0, "{table}");
    }
    let quick: Vec<i32> = sqlx::query_scalar("SELECT number FROM character_quick_slots ORDER BY position").fetch_all(&db.pool).await.unwrap();
    assert_eq!(quick, [9], "the quick slot that pointed at the deleted bag slot went with it");
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.ext_log WHERE action = 'force-delete'", db.s.content)).await, 4);
    assert_eq!(item(&db, 100212).await["itemname"], json!("???177"));
    db.drop().await;
}

#[tokio::test]
async fn other_data_that_points_at_the_rows_stops_the_removal_even_with_force() {
    let Some(db) = seeded(true).await else { return };
    install(&db, &Folder::new()).await;
    let (d, w) = (&db.s.defs, &db.s.world);
    run(&db.pool, format!("INSERT INTO {d}.product_list (rowid, productname, productcode, buycost, itemid1, itemcount1, itemid3, itemcount3) VALUES (2, 'Kill Pack', 'P2041', 5, 100108, 1, 100212, 2)")).await;
    run(&db.pool, format!("INSERT INTO {w}.npc_shop_items (npc_type_id, seq, item_type, item_type_id) VALUES (1, 0, 100, 213)")).await;
    run(&db.pool, format!("INSERT INTO {w}.quests (id, name, start_items, end_items) VALUES (7, 'Fetch', '{{0,0,0,0,0,0,0,0,0}}', '{{0,0,0,0,0,0,100,214,1}}')")).await;
    run(&db.pool, format!("INSERT INTO {d}.items (rowid, itemid, itemname, type, typeid, range, attacktime, special) VALUES (20, 100300, 'Foreign remedy', 100, 300, 200, 2, 0)")).await;
    run(&db.pool, format!("INSERT INTO {d}.mobs (rowid, mobid, mobname, level, hp, questitemid) VALUES (30, 77, 'Carrier', 1, 5, 100212) ON CONFLICT DO NOTHING")).await;
    db.draft(set("item_grade", "100213", "grade", json!(8))).await;
    let applied = db.apply("grade").await;
    for force in [false, true] {
        let message = refusal(db.editor.ext_remove("kb", force, |_| Ok(true)).await.unwrap_err());
        for needle in [
            "defs.product_list P2041 itemid3 = 100212",
            "world.npc_shop_items 1/0 item_type/item_type_id = 100213",
            "world.quests 7 end_items = 100214",
            "defs.items 100300 gives the skill 200/2",
            &format!("edit item_grade 100213 grade (change set #{})", applied.change_set),
            "--force does not clear references",
        ] {
            assert!(message.contains(needle), "force {force}: wanted {needle:?} in {message}");
        }
        assert!(!message.contains("100108"), "the stone 100/108 is not an owned row: {message}");
    }
    // each reference gone, the removal passes
    run(&db.pool, format!("DELETE FROM {}.product_list WHERE productcode = 'P2041'", db.s.defs)).await;
    run(&db.pool, format!("DELETE FROM {}.npc_shop_items", db.s.world)).await;
    run(&db.pool, format!("DELETE FROM {}.quests", db.s.world)).await;
    run(&db.pool, format!("DELETE FROM {}.items WHERE itemid = 100300", db.s.defs)).await;
    run(&db.pool, format!("DELETE FROM {}.mobs WHERE mobid = 77", db.s.defs)).await;
    db.editor.undo(applied.change_set, "tester").await.unwrap();
    let lines = db.editor.ext_remove("kb", false, |_| Ok(true)).await.unwrap();
    assert!(has(&lines, "removed kb 1.0.0"), "{lines:#?}");
    db.drop().await;
}

#[tokio::test]
async fn an_upgrade_restores_dropped_keys_rewrites_changed_ones_claims_new_ones_and_keeps_since_patch_where_nothing_changed() {
    let Some(db) = seeded(true).await else { return };
    let before = hashes(&db).await;
    install(&db, &Folder::new()).await;
    run(&db.pool, format!("UPDATE {c}.client_items SET since_patch = 109; UPDATE {c}.client_skills SET since_patch = 109", c = db.s.content)).await;
    let installed_hashes = hashes(&db).await;

    // the same or a lower version is refused
    let same = Folder::new();
    let message = refusal(db.editor.ext_upgrade(same.path()).await.unwrap_err());
    assert!(message.contains("an upgrade needs a higher version"), "{message}");
    let lower = Folder::bare("kb", "0.9.0", "", "");
    assert!(refusal(db.editor.ext_upgrade(lower.path()).await.unwrap_err()).contains("higher version"));
    let nobody = Folder::bare("other", "1.0.0", "", "");
    assert!(refusal(db.editor.ext_upgrade(nobody.path()).await.unwrap_err()).contains("other is not installed"));

    // 1.1.0: 100/214 dropped, 100/213 reworded, 100/215 new, 100/212 and the skills as they were
    let v2 = || {
        Folder::new()
            .edit("extension.toml", "version = \"1.0.0\"", "version = \"1.1.0\"")
            .edit("extension.toml", "\"100/212-214\"", "\"100/212-213\", \"100/215\"")
            .edit("rows.toml", "[[item]]\nid = \"100/214\"", "[[item]]\nid = \"100/215\"")
            .edit("rows.toml", "itemname = \"Kill Boost Stone Lv3\", range = 200, attacktime = 3", "itemname = \"Kill Boost Stone Lv4\", range = 200, attacktime = 3")
            .edit("rows.toml", "description = \"Player kills count 5 times for 1 hour.\" }\n\n[[skill]]", "description = \"Player kills count 5 times for 1 hour.\" }\n\n[[skill]]")
            .edit("rows.toml", "client = { name = \"Kill Boost Stone Lv2\", look = \"100/108\", description = \"Player kills count 5 times for 1 hour.\" }", "client = { name = \"Kill Boost Stone Lv2\", look = \"100/108\", description = \"Five times, one hour.\" }")
    };
    // 100/215 is a new place (an add), 100/214 goes back to the placeholder: a holder of 100/214 stops the upgrade
    let (_, character) = make_holder(&db).await;
    sqlx::query("INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, count) VALUES ($1, 1, 1, 100214, 100, 214, 1)").bind(character).execute(&db.pool).await.unwrap();
    let message = refusal(db.editor.ext_upgrade(v2().path()).await.unwrap_err());
    assert!(message.contains("kb 1.1.0 no longer lists rows that players hold") && message.contains("1 × 100/214 in bag 1, slot 1"), "{message}");
    assert_eq!(installed_hashes, hashes(&db).await, "a refused upgrade writes nothing");
    sqlx::query("DELETE FROM character_items").execute(&db.pool).await.unwrap();

    let lines = db.editor.ext_upgrade(v2().path()).await.unwrap();
    assert!(has(&lines, "upgrading kb 1.0.0 -> 1.1.0") && has(&lines, "1 rows no longer listed were restored"), "{lines:#?}");
    assert_eq!(item(&db, 100214).await["itemname"], json!("???177"), "the dropped key is back to the placeholder");
    assert_eq!(item(&db, 100215).await["itemname"], json!("Kill Boost Stone Lv4"), "the new key is claimed (an add)");
    assert_eq!(item(&db, 100212).await["itemname"], json!("Kill Boost Stone"));
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.ext_rows WHERE extension = 'kb'", db.s.content)).await, 6);
    assert_eq!(db.value_text(&format!("{}.extensions", db.s.content), "version", "name = 'kb'").await, "1.1.0");
    assert_eq!(since(&db, "client_items", "type = 100 AND typeid = 212").await, json!(109), "nothing changed: shipped in patch 109");
    assert_eq!(since(&db, "client_items", "type = 100 AND typeid = 213").await, Value::Null, "reworded: due for a new patch");
    assert_eq!(since(&db, "client_items", "type = 100 AND typeid = 215").await, Value::Null, "new");
    assert_eq!(since(&db, "client_skills", "skill_id = 200 AND level = 1").await, json!(109));
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.client_items WHERE typeid = 214", db.s.content)).await, 0);
    assert_eq!(db.editor.content_check().await.unwrap(), Vec::<String>::new());
    // a rebuild agrees with the upgraded state
    let upgraded = hashes(&db).await;
    db.editor.content_rebuild(&[Side::Defs]).await.unwrap();
    assert_eq!(upgraded, hashes(&db).await);
    // back to nothing
    db.editor.ext_remove("kb", false, |_| Ok(true)).await.unwrap();
    assert_eq!(before, hashes(&db).await);
    db.drop().await;
}

#[tokio::test]
async fn disable_and_enable_flip_the_flag_and_leave_the_rows() {
    let Some(db) = seeded(false).await else { return };
    install(&db, &Folder::new()).await;
    let rows = hashes(&db).await;
    let lines = db.editor.ext_set_enabled("kb", false).await.unwrap();
    assert!(has(&lines, "kb disabled: its settings defaults stop counting; its rows stay") && has(&lines, "restart the game servers"), "{lines:#?}");
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.extensions WHERE NOT enabled", db.s.content)).await, 1);
    assert_eq!(rows, hashes(&db).await);
    assert_eq!(db.editor.ext_set_enabled("kb", false).await.unwrap(), ["kb is disabled already"]);
    db.editor.ext_set_enabled("kb", true).await.unwrap();
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.ext_log WHERE action IN ('enable', 'disable')", db.s.content)).await, 2);
    assert!(refusal(db.editor.ext_set_enabled("nope", true).await.unwrap_err()).contains("nope is not installed"));
    // a rebuild writes disabled extensions' rows too (decision 4)
    db.editor.ext_set_enabled("kb", false).await.unwrap();
    db.editor.content_rebuild(&[Side::Defs]).await.unwrap();
    assert_eq!(rows, hashes(&db).await);
    db.drop().await;
}

#[tokio::test]
async fn check_lists_what_an_install_would_change_and_writes_nothing() {
    let Some(db) = seeded(false).await else { return };
    let before = hashes(&db).await;
    let lines = db.editor.ext_check(Folder::new().path()).await.unwrap();
    assert!(has(&lines, "checking the install of kb 1.0.0") && has(&lines, "rows.toml [[item]] #1 (100/212) changes defs.items 100212 (set: attacktime, itemname, range)"), "{lines:#?}");
    assert!(has(&lines, "nothing was written"), "{lines:#?}");
    assert_eq!(before, hashes(&db).await);
    assert_eq!(count_in_content(&db, "extensions").await, 0);
    db.drop().await;
}

#[tokio::test]
async fn the_builtin_group_counts_are_the_client_schemas_when_it_exists() {
    dotenvy::dotenv().ok();
    let Ok(url) = std::env::var("DATABASE_URL") else { return };
    let pool = sqlx::postgres::PgPoolOptions::new().max_connections(1).connect(&url).await.unwrap();
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('client.items') IS NOT NULL").fetch_one(&pool).await.unwrap();
    if !exists {
        eprintln!("no client schema here: the built-in group counts are not compared");
        return;
    }
    let counts: Vec<(i16, i64)> = sqlx::query_as("SELECT grp, count(*) FROM client.items GROUP BY grp ORDER BY grp").fetch_all(&pool).await.unwrap();
    let mut want = [0i64; 100];
    for (group, n) in counts {
        want[group as usize - 1] = n;
    }
    let have: Vec<i64> = data_editor::ext::ids::ITEM_GROUP_RECORDS.iter().map(|&n| i64::from(n)).collect();
    assert_eq!(have, want, "ext/ids.rs ITEM_GROUP_RECORDS is not client.items' group sizes");
}
