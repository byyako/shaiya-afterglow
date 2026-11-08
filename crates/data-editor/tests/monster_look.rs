//! `[[monster_look]]` entries and the `monsters` ids of an extension (`docs/plans/ext-4-pets.md` section 6.4) against a real PostgreSQL
//! inside throwaway schemas: install, check, upgrade, remove, show, list and the refusals. Skipped (and says so) without DATABASE_URL.

mod common;

use std::path::{Path, PathBuf};

use common::*;
use serde_json::json;

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str, version: &str, monsters: &str, rows: &str) -> Self {
        let to = std::env::temp_dir().join(format!("monster-look-test-{:08x}", rand::random::<u32>()));
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(
            to.join("extension.toml"),
            format!(
                "format = 1\nname = \"{name}\"\nversion = \"{version}\"\ntitle = \"T\"\ndescription = \"D\"\n[ids]\nmonsters = [{monsters}]\n"
            ),
        )
        .unwrap();
        std::fs::write(to.join("rows.toml"), rows).unwrap();
        Self(to)
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

const LOOK: &str = r#"
[[monster_look]]
of = "1-10"
base = 3000
day = { "0" = 6, "2" = 0 }
set = { size = 0, hp = 1 }
note = "small copies"
"#;

async fn seeded() -> Option<Db> {
    let db = Db::create().await?;
    db.seed_kill_boost().await;
    db.freeze("both").await;
    Some(db)
}

/// A scratch `client.monsters` with the records 0 to `count - 1`, the longest name 20 bytes (21 with its NUL, as the original's are
/// stored without the NUL: the column holds the text).
async fn client_monsters(db: &Db, count: i32) {
    let client = &db.s.client;
    run(&db.pool, format!("CREATE SCHEMA {client}; CREATE TABLE {client}.monsters (mob_id smallint PRIMARY KEY, name text NOT NULL)"))
        .await;
    run(
        &db.pool,
        format!("INSERT INTO {client}.monsters SELECT i, CASE WHEN i = 0 THEN 'Error Monster' ELSE repeat('x', 12) END FROM generate_series(0, {}) i", count - 1),
    )
    .await;
}

fn has(lines: &[String], needle: &str) -> bool {
    lines.iter().any(|l| l.contains(needle))
}

async fn rows(db: &Db) -> i64 {
    db.count(format!("SELECT count(*) FROM {}.client_monsters", db.s.content)).await
}

#[tokio::test]
async fn a_monster_look_installs_checks_lists_shows_and_removes() {
    let Some(db) = seeded().await else { return };
    client_monsters(&db, 12).await;
    let v1 = Folder::new("pets", "1.0.0", "\"3001-3010\"", LOOK);
    let checked = db.editor.ext_check(v1.path()).await.unwrap();
    assert!(has(&checked, "monster_look 1-10 → 3001-3010, day mapped, 2 fields set"), "{checked:#?}");
    assert!(has(&checked, "10 client rows") && has(&checked, "nothing was written"), "{checked:#?}");
    assert_eq!(rows(&db).await, 0, "a check writes nothing");

    let lines = db.editor.ext_install(v1.path(), false).await.unwrap();
    assert!(has(&lines, "monster_look 1-10 → 3001-3010, day mapped, 2 fields set"), "{lines:#?}");
    assert!(has(&lines, "next: run tools/sdata/client_patch.py check"), "{lines:#?}");
    assert_eq!(rows(&db).await, 10);
    let row = db.scalar(format!("SELECT * FROM {}.client_monsters WHERE id = 3005", db.s.content)).await;
    assert_eq!(
        (row["extension"].clone(), row["copy_of"].clone(), row["since_patch"].clone(), row["note"].clone()),
        (json!("pets"), json!(5), json!(null), json!("small copies"))
    );
    assert_eq!(row["fields"], json!({ "size": 0, "hp": 1, "day": { "0": 6, "2": 0 } }), "the day map is kept for the generator");
    let range =
        db.scalar(format!("SELECT kind, major, minor_low, minor_high FROM {}.ext_ranges WHERE extension = 'pets'", db.s.content)).await;
    assert_eq!(range, json!({ "kind": "monster", "major": 0, "minor_low": 3001, "minor_high": 3010 }));

    let list = db.editor.ext_list().await.unwrap();
    let line = list.iter().find(|l| l.starts_with("pets ")).expect("listed");
    assert_eq!(line.split_whitespace().nth(4), Some("10"), "10 client rows: {line}");
    let shown = db.editor.ext_show("pets").await.unwrap();
    assert!(has(&shown, "ids: monster 3001-3010"), "{shown:#?}");
    assert!(has(&shown, "client monster_look 1-10 → 3001-3010, day mapped, 2 fields set (since patch -)"), "{shown:#?}");
    assert_eq!(shown.iter().filter(|l| l.contains("monster_look")).count(), 1, "one line, not ten");

    // a second installed extension may not take the same records
    let other = Folder::new("rival", "1.0.0", "\"3005-3020\"", "");
    let error = db.editor.ext_install(other.path(), false).await.unwrap_err().to_string();
    assert!(error.contains("monster ids 3005-3020 overlap 3001-3010 of the extension pets"), "{error}");

    // remove: the rows and the ids go; a record an earlier patch carried makes a patch due
    run(&db.pool, format!("UPDATE {}.client_monsters SET since_patch = 111", db.s.content)).await;
    let removed = db.editor.ext_remove("pets", false, |_| Ok(true)).await.unwrap();
    assert!(has(&removed, "a client patch is due: 10 client rows were in an earlier patch"), "{removed:#?}");
    assert_eq!(rows(&db).await, 0);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.ext_ranges", db.s.content)).await, 0);
}

#[tokio::test]
async fn an_upgrade_keeps_since_patch_for_a_record_that_did_not_change() {
    let Some(db) = seeded().await else { return };
    let v1 = Folder::new("pets", "1.0.0", "\"3001-3010\"", LOOK);
    db.editor.ext_install(v1.path(), false).await.unwrap();
    run(&db.pool, format!("UPDATE {}.client_monsters SET since_patch = 111", db.s.content)).await;
    // the same look with the first five records only, and the note changed for none
    let v2 = Folder::new("pets", "1.1.0", "\"3001-3010\"", &LOOK.replace("1-10", "1-5"));
    db.editor.ext_upgrade(v2.path()).await.unwrap();
    assert_eq!(rows(&db).await, 5);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.client_monsters WHERE since_patch = 111", db.s.content)).await, 5);
    // a changed field starts again
    let v3 = Folder::new("pets", "1.2.0", "\"3001-3010\"", &LOOK.replace("1-10", "1-5").replace("hp = 1", "hp = 2"));
    db.editor.ext_upgrade(v3.path()).await.unwrap();
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.client_monsters WHERE since_patch IS NULL", db.s.content)).await, 5);
    let shown = db.editor.ext_show("pets").await.unwrap();
    assert!(has(&shown, "client monster_look 1-5 → 3001-3005, day mapped, 2 fields set (since patch -)"), "{shown:#?}");
}

#[tokio::test]
async fn the_refusals_name_the_entry_and_the_reason() {
    let Some(db) = seeded().await else { return };
    // outside [ids] monsters (no database needed)
    let outside = Folder::new("pets", "1.0.0", "\"3001-3005\"", LOOK);
    let error = db.editor.ext_check(outside.path()).await.unwrap_err().to_string();
    assert!(
        error.contains(
            "rows.toml [[monster_look]] #1: record 3006 (a copy of 6) lies outside the ids [ids] declares for monsters (3001-3005)"
        ),
        "{error}"
    );
    // two entries making the same record
    let twice = Folder::new("pets", "1.0.0", "\"3001-3010\"", &format!("{LOOK}{}", LOOK.replace("1-10", "10-12")));
    let error = db.editor.ext_check(twice.path()).await.unwrap_err().to_string();
    assert!(error.contains("[[monster_look]] #2: makes record 3010 (a copy of 10) that [[monster_look]] #1 makes already"), "{error}");
    // a record that is a server monster
    run(
        &db.pool,
        format!(
            "INSERT INTO {}.mobs (rowid, mobid, mobname, level, hp, exp, money1, money2) VALUES (50, 3005, 'Placeholder', 1, 1, 1, 0, 0)",
            db.s.defs
        ),
    )
    .await;
    let clash = Folder::new("pets", "1.0.0", "\"3001-3010\"", LOOK);
    let error = db.editor.ext_check(clash.path()).await.unwrap_err().to_string();
    assert!(error.contains("record 3005 is a monster of the server (defs.mobs)"), "{error}");
    run(&db.pool, format!("DELETE FROM {}.mobs WHERE mobid = 3005", db.s.defs)).await;
    // a copy of a record the client's file does not have
    client_monsters(&db, 6).await;
    let error = db.editor.ext_check(clash.path()).await.unwrap_err().to_string();
    assert!(error.contains("[[monster_look]] #1: of: 5 of the 10 records are not in the client's Monster.SData"), "{error}");
    // a name longer than the longest of the original (12 bytes here)
    client_monsters_names(&db).await;
    let long = Folder::new(
        "pets",
        "1.0.0",
        "\"3001-3002\"",
        "[[monster_look]]\nof = \"1-2\"\nbase = 3000\nset = { name = \"A name that is far too long\" }\n",
    );
    let error = db.editor.ext_check(long.path()).await.unwrap_err().to_string();
    assert!(error.contains("set name: 27 bytes is longer than the longest monster name in the original (13)"), "{error}");
    assert_eq!(rows(&db).await, 0);
}

async fn client_monsters_names(db: &Db) {
    run(
        &db.pool,
        format!(
            "DELETE FROM {}.monsters WHERE mob_id > 5; UPDATE {}.monsters SET name = repeat('y', 14) WHERE mob_id = 1",
            db.s.client, db.s.client
        ),
    )
    .await;
}
