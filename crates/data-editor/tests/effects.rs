//! Item effects and skill abilities of extensions (`docs/plans/ext-3-effects.md` section 4) against a real PostgreSQL inside throwaway
//! schemas: install, check, upgrade, remove, show and list, the refusals, the client rule and the disabled-fallback warning. Skipped
//! (and says so) without DATABASE_URL.

mod common;

use std::path::{Path, PathBuf};

use common::*;
use serde_json::{Value, json};

/// A temporary extension folder, removed afterwards.
struct Folder(PathBuf);

const MANIFEST: &str = "[ids]\nitems = [\"100/108\", \"100/212-214\", \"95/3\"]\nskills = [\"200/1\"]\n";

const STONE: &str = r#"
[[item]]
id = "100/212"
copy = { from = "100/108", columns = ["reqlevel", "country", "grow", "special", "slot", "quality", "buy", "sell", "grade", "drop", "server", "count"] }
set = { itemname = "Level Stone", range = 0, attacktime = 0 }
effect = { type = "level_up", levels = 1 }
client = { name = "Level Stone", look = "100/108", description = "Raises your level by one." }

[[skill]]
id = "200/1"
copy = { from = "250/1" }
set = { skillname = "Kill Boost", keeptime = 1, fixrange = 1, abilitytype1 = 0, abilityvalue1 = 0, abilitytype2 = 0, abilityvalue2 = 0, abilitytype3 = 0, abilityvalue3 = 0 }
abilities = [{ type = "kill_boost", times = 2 }]
client = { name = "Kill Boost", look = "250/1", description = "Player kills count double." }
"#;

impl Folder {
    fn new(version: &str, rows: &str) -> Self {
        let to = std::env::temp_dir().join(format!("effects-test-{:08x}", rand::random::<u32>()));
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(to.join("extension.toml"), format!("format = 1\nname = \"fx\"\nversion = \"{version}\"\ntitle = \"T\"\ndescription = \"D\"\n{MANIFEST}")).unwrap();
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

/// The scratch definitions of the kill-boost fixture, a Lapisia placeholder (95/3), frozen as the
/// originals.
async fn seeded(full: bool) -> Option<Db> {
    let db = if full { Db::create_full().await } else { Db::create().await }?;
    db.seed_kill_boost().await;
    let d = &db.s.defs;
    run(&db.pool, format!("INSERT INTO {d}.items (rowid, itemid, itemname, type, typeid, special, count, range, attacktime) VALUES (20, 95003, '???', 95, 3, 0, 1, 0, 0)")).await;
    db.freeze("both").await;
    Some(db)
}

fn has(lines: &[String], needle: &str) -> bool {
    lines.iter().any(|l| l.contains(needle))
}

async fn count(db: &Db, table: &str) -> i64 {
    db.count(format!("SELECT count(*) FROM {}.{table}", db.s.content)).await
}

#[tokio::test]
async fn an_effect_and_an_ability_install_check_upgrade_and_remove() {
    let Some(db) = seeded(true).await else { return };
    let v1 = Folder::new("1.0.0", STONE);
    // A check prints each effect and ability and writes nothing.
    let checked = db.editor.ext_check(v1.path()).await.unwrap();
    assert!(has(&checked, "100/212 level_up levels = 1") && has(&checked, "200/1 kill_boost times = 2"), "{checked:#?}");
    assert!(has(&checked, "2 item effects and skill abilities") && has(&checked, "restart the game servers"), "{checked:#?}");
    assert_eq!((count(&db, "ext_item_effects").await, count(&db, "ext_skill_abilities").await), (0, 0), "a check writes nothing");

    let lines = db.editor.ext_install(v1.path(), false).await.unwrap();
    assert!(has(&lines, "100/212 level_up levels = 1") && has(&lines, "200/1 kill_boost times = 2"), "{lines:#?}");
    let stored = db.scalar(format!("SELECT effect AS v FROM {}.ext_item_effects WHERE extension = 'fx' AND item_id = 100212", db.s.content)).await;
    assert_eq!(stored["v"], json!({ "type": "level_up", "levels": 1 }));
    let stored = db.scalar(format!("SELECT abilities AS v FROM {}.ext_skill_abilities WHERE extension = 'fx' AND skill_id = 200 AND level = 1", db.s.content)).await;
    assert_eq!(stored["v"], json!([{ "type": "kill_boost", "times": 2 }]));
    // The defs rows carry no trace of them (the client's data and `content hashes defs` are as without the effects).
    let stone = db.scalar(format!("SELECT to_jsonb(t) AS v FROM {}.items t WHERE itemid = 100212", db.s.defs)).await["v"].clone();
    assert_eq!((stone["itemname"].clone(), stone["special"].clone(), stone["range"].clone()), (json!("Level Stone"), json!(0), json!(0)));

    // list and show
    let list = db.editor.ext_list().await.unwrap();
    assert!(list[0].contains("effects"), "{list:#?}");
    let row = list.iter().find(|l| l.starts_with("fx ")).expect("fx is listed");
    let numbers: Vec<&str> = row.split_whitespace().collect();
    assert_eq!(numbers[numbers.len() - 4], "2", "rows 2, client 2, effects 2: {row}");
    let shown = db.editor.ext_show("fx").await.unwrap();
    assert!(has(&shown, "effect 100/212: level_up levels = 1") && has(&shown, "abilities 200/1: kill_boost times = 2"), "{shown:#?}");

    // an upgrade replaces both
    let v2 = Folder::new(
        "1.1.0",
        &STONE.replace("levels = 1 }", "levels = 3, max = 50 }").replace("times = 2", "times = 5").replace("abilities = [", "abilities = [{ type = \"kill_boost\", times = 3 }, "),
    );
    let lines = db.editor.ext_upgrade(v2.path()).await.unwrap();
    assert!(has(&lines, "100/212 level_up levels = 3, max = 50") && has(&lines, "200/1 kill_boost times = 5"), "{lines:#?}");
    let stored = db.scalar(format!("SELECT effect AS v FROM {}.ext_item_effects WHERE item_id = 100212", db.s.content)).await;
    assert_eq!(stored["v"], json!({ "type": "level_up", "levels": 3, "max": 50 }));
    assert_eq!((count(&db, "ext_item_effects").await, count(&db, "ext_skill_abilities").await), (1, 1), "replaced, not added to");
    let stored = db.scalar(format!("SELECT abilities AS v FROM {}.ext_skill_abilities WHERE skill_id = 200", db.s.content)).await;
    assert_eq!(stored["v"].as_array().map(Vec::len), Some(2));

    // an upgrade that no longer lists the effect takes it out
    let v3 = Folder::new("1.2.0", &STONE.replace("effect = { type = \"level_up\", levels = 1 }\n", "").replace("abilities = [{ type = \"kill_boost\", times = 2 }]\n", ""));
    db.editor.ext_upgrade(v3.path()).await.unwrap();
    assert_eq!((count(&db, "ext_item_effects").await, count(&db, "ext_skill_abilities").await), (0, 0));

    // remove: back to the v2 folder first, so the cascade has something to take
    let v4 = Folder::new("1.3.0", STONE);
    db.editor.ext_upgrade(v4.path()).await.unwrap();
    assert_eq!(count(&db, "ext_item_effects").await, 1);
    let lines = db.editor.ext_remove("fx", false, |_| Ok(true)).await.unwrap();
    assert!(has(&lines, "removed fx 1.3.0"), "{lines:#?}");
    assert_eq!((count(&db, "ext_item_effects").await, count(&db, "ext_skill_abilities").await), (0, 0), "the cascade took them");
    db.drop().await;
}

#[tokio::test]
async fn a_wrong_row_and_a_missing_client_part_are_refused_naming_the_entry() {
    let Some(db) = seeded(false).await else { return };
    let head = "[[item]]\nid = \"100/212\"\ncopy = { from = \"100/108\" }\nclient = { name = \"A\", look = \"100/108\", description = \"b\" }\n";
    // a level_up effect on the Lapisia's row (95/3, an original of item type 95)
    let wrong = Folder::new(
        "1.0.0",
        &format!("{head}\n[[item]]\nid = \"95/3\"\neffect = {{ type = \"level_up\", levels = 1 }}\nclient = {{ name = \"B\", look = \"100/108\", description = \"b\" }}\n"),
    );
    let message = refusal(db.editor.ext_install(wrong.path(), false).await.unwrap_err());
    assert!(message.contains("rows.toml [[item]] #2 (95/3)") && message.contains("effect level_up needs an item the core uses (item type 25, 44 or 100); this item is type 95"), "{message}");
    // an enchant effect on a stone: the computed row is item type 100
    let wrong = Folder::new(
        "1.0.0",
        &format!("{head}\n[[item]]\nid = \"100/213\"\ncopy = {{ from = \"100/108\" }}\neffect = {{ type = \"enchant\", safe = true }}\nclient = {{ name = \"B\", look = \"100/108\", description = \"b\" }}\n"),
    );
    let message = refusal(db.editor.ext_install(wrong.path(), false).await.unwrap_err());
    assert!(message.contains("rows.toml [[item]] #2 (100/213)") && message.contains("effect enchant needs a Lapisia (item type 95); this item is type 100"), "{message}");
    // a recreate effect on a row that is not special 62
    let wrong = Folder::new(
        "1.0.0",
        &format!("{head}\n[[item]]\nid = \"100/213\"\ncopy = {{ from = \"100/108\" }}\neffect = {{ type = \"recreate\", stats = [\"str\"] }}\nclient = {{ name = \"B\", look = \"100/108\", description = \"b\" }}\n"),
    );
    let message = refusal(db.editor.ext_install(wrong.path(), false).await.unwrap_err());
    assert!(message.contains("(100/213)") && message.contains("needs a Recreation Rune (special 62)"), "{message}");
    // a parse error is found before the database is asked, with the entry in it
    let wrong = Folder::new("1.0.0", &format!("{head}\n[[item]]\nid = \"100/213\"\neffect = {{ type = \"level_up\", levels = 0 }}\n"));
    let message = refusal(db.editor.ext_install(wrong.path(), false).await.unwrap_err());
    assert!(message.contains("rows.toml [[item]] #2 (100/213): effect: levels: 0 is not 1 to 59"), "{message}");
    // an effect on an original row that no column changes still needs the client's text (or server_only)
    let bare = Folder::new("1.0.0", "[[item]]\nid = \"100/108\"\neffect = { type = \"level_up\", levels = 1 }\n");
    let message = refusal(db.editor.ext_install(bare.path(), false).await.unwrap_err());
    assert!(message.contains("rows.toml [[item]] #1 (100/108)") && message.contains("needs `client = { … }`"), "{message}");
    let reasoned = Folder::new("1.0.0", "[[item]]\nid = \"100/108\"\neffect = { type = \"level_up\", levels = 1 }\nserver_only = \"the client text is the stone's own\"\n");
    db.editor.ext_install(reasoned.path(), false).await.unwrap();
    assert_eq!(count(&db, "ext_item_effects").await, 1);
    db.drop().await;
}

#[tokio::test]
async fn an_item_that_falls_back_to_a_core_effect_is_warned_about() {
    let Some(db) = seeded(false).await else { return };
    // 100/212 would be a potion when its extension is disabled; 100/213 is a refusal (nothing to say).
    let potion = "[[item]]\nid = \"100/212\"\ncopy = { from = \"100/108\" }\nset = { special = 0, range = 0, attacktime = 0, consthp = 50 }\neffect = { type = \"level_up\", levels = 1 }\nclient = { name = \"A\", look = \"100/108\", description = \"b\" }\n";
    let refusing = "[[item]]\nid = \"100/213\"\ncopy = { from = \"100/108\" }\nset = { special = 0, range = 0, attacktime = 0 }\neffect = { type = \"level_up\", levels = 1 }\nclient = { name = \"A\", look = \"100/108\", description = \"b\" }\n";
    let f = Folder::new("1.0.0", &format!("{potion}\n{refusing}"));
    let checked = db.editor.ext_check(f.path()).await.unwrap();
    assert!(has(&checked, "warning: rows.toml [[item]] #1 (100/212): when the extension is disabled, 100/212 is a potion"), "{checked:#?}");
    assert!(!checked.iter().any(|l| l.contains("100/213 is")), "a refusal needs no warning: {checked:#?}");
    db.drop().await;
}

#[tokio::test]
async fn a_changed_effect_is_a_changed_row_for_ext_scripts() {
    let Some(db) = seeded(false).await else { return };
    let f = Folder::new("1.0.0", STONE);
    db.editor.ext_install(f.path(), false).await.unwrap();
    // the same folder: nothing differs
    db.editor.ext_scripts(f.path()).await.unwrap();
    let changed = Folder::new("1.0.0", &STONE.replace("levels = 1", "levels = 2"));
    let message = refusal(db.editor.ext_scripts(changed.path()).await.unwrap_err());
    assert!(message.contains("use `ext upgrade` with a higher version") && message.contains("item effects"), "{message}");
    let changed = Folder::new("1.0.0", &STONE.replace("times = 2", "times = 3"));
    let message = refusal(db.editor.ext_scripts(changed.path()).await.unwrap_err());
    assert!(message.contains("skill abilities"), "{message}");
    // the stored effect is as it was
    let stored: Value = db.scalar(format!("SELECT effect AS v FROM {}.ext_item_effects", db.s.content)).await["v"].clone();
    assert_eq!(stored, json!({ "type": "level_up", "levels": 1 }));
    db.drop().await;
}
