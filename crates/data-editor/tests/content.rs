//! `data-editor content …` against a real PostgreSQL inside throwaway schemas (see `common`): rebuild, check, origin, status, hashes.
//! Skipped (and says so) when DATABASE_URL is not set.

mod common;

use common::*;
use data_editor::registry::Side;
use serde_json::json;

async fn hashes(db: &Db) -> Vec<String> {
    db.editor.content_hashes(&[db.s.defs.clone(), db.s.world.clone()]).await.unwrap()
}

/// The item 30071 as the extension "t1" changes it: a new name.
async fn blade(db: &Db) -> serde_json::Value {
    let mut row = db.scalar(format!("SELECT to_jsonb(t) AS v FROM {}.items t WHERE itemid = 30071", db.s.defs)).await["v"].clone();
    row["itemname"] = json!("Blade");
    row
}

#[tokio::test]
async fn a_rebuild_gives_the_originals_the_extension_rows_and_the_edits_and_twice_the_same() {
    let Some(db) = Db::create().await else { return };
    db.freeze("both").await;
    let row = blade(&db).await;
    db.ext_row("t1", "defs.items", "30071", "change", row).await;
    db.draft(set("item_grade", "30071", "grade", json!(77))).await;
    db.apply("raise the grade").await;
    // a hand edit that the rebuild replaces, and a row that is not in the originals
    run(&db.pool, format!("UPDATE {}.mobs SET hp = 1 WHERE mobid = 10", db.s.defs)).await;
    run(&db.pool, format!("INSERT INTO {}.mobs (rowid, mobid, mobname, level, hp, exp, money1, money2) VALUES (9, 99, 'Extra', 1, 1, 1, 1, 1)", db.s.defs)).await;

    let first = db.editor.content_rebuild(&[Side::Defs, Side::World]).await.unwrap();
    assert_eq!(first.iter().map(|r| r.part.as_str()).collect::<Vec<_>>(), ["defs", "world"]);
    assert_eq!(first[0].ext_rows, 1);
    assert!(first.iter().all(|r| !r.created && r.report.problems.is_empty()), "{first:#?}");
    assert_eq!(db.value("items", "itemname", "itemid = 30071").await, json!("Blade"), "the extension's row");
    assert_eq!(db.value("items", "grade", "itemid = 30071").await, json!(77), "the edit on top");
    assert_eq!(db.value("mobs", "hp", "mobid = 10").await, json!(100), "the hand edit is replaced by the original");
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.mobs WHERE mobid = 99", db.s.defs)).await, 0, "so is the hand-made row");
    assert_eq!(db.value("items", "itemname", "itemid = 30071").await, json!("Blade"));
    // the frozen copy is as imported
    let frozen = db.scalar(format!("SELECT itemname AS v FROM {}.items WHERE itemid = 30071", db.s.original_defs)).await;
    assert_eq!(frozen["v"], json!("Sword"));
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.imports WHERE rebuilt_at IS NOT NULL", db.s.content)).await, 2);

    let hashes_once = hashes(&db).await;
    let second = db.editor.content_rebuild(&[Side::Defs, Side::World]).await.unwrap();
    assert!(second.iter().all(|r| r.report.problems.is_empty()), "{second:#?}");
    assert_eq!(hashes_once, hashes(&db).await, "a second rebuild writes the same bytes");
    assert!(hashes_once.iter().any(|l| l.starts_with(&format!("{}.items|1|", db.s.defs))), "{hashes_once:?}");
    // the world's identity sequences continue past the highest id
    run(&db.pool, format!("INSERT INTO {}.mob_areas (map, seq, x1, y1, z1, x2, y2, z2) VALUES (2, 77, 1, 0, 1, 2, 0, 2)", db.s.world)).await;
    db.drop().await;
}

#[tokio::test]
async fn a_part_without_an_import_is_refused_and_a_missing_live_schema_is_made() {
    let Some(db) = Db::create().await else { return };
    let message = refusal(db.editor.content_rebuild(&[Side::Defs]).await.unwrap_err());
    assert!(message.contains("defs has no import record") && message.contains("tools/setup.sh"), "{message}");
    assert_eq!(db.value("mobs", "hp", "mobid = 10").await, json!(100), "a refused rebuild writes nothing");

    db.freeze_from_ddl().await;
    run(&db.pool, format!("DROP SCHEMA {} CASCADE", db.s.defs)).await;
    let result = db.editor.content_rebuild(&[Side::Defs]).await.unwrap();
    assert!(result[0].created);
    assert_eq!(result[0].tables.iter().find(|(t, _)| t == "exp_defs").map(|(_, n)| *n), Some(2));
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.exp_defs", db.s.defs)).await, 2);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.items", db.s.defs)).await, 0);
    // the world has no record: still refused, with the part named
    let message = refusal(db.editor.content_rebuild(&[Side::World]).await.unwrap_err());
    assert!(message.contains("world has no import record"), "{message}");
    db.drop().await;
}

#[tokio::test]
async fn into_scratch_schemas_is_rolled_back() {
    let Some(db) = Db::create().await else { return };
    db.freeze("both").await;
    run(&db.pool, format!("UPDATE {}.mobs SET hp = 1 WHERE mobid = 10", db.s.defs)).await;
    let before = hashes(&db).await;
    let result = db.editor.content_rebuild_into(&[Side::Defs], "scratch_x").await.unwrap();
    assert!(result[0].created && result[0].tables.iter().any(|(t, n)| t == "mobs" && *n == 4));
    assert_eq!(before, hashes(&db).await, "the live tables are untouched");
    assert_eq!(db.count("SELECT count(*) FROM pg_namespace WHERE nspname LIKE 'scratch_x%'".into()).await, 0, "and the scratch schemas are gone");
    db.drop().await;
}

#[tokio::test]
async fn check_is_clean_after_a_rebuild_and_finds_a_hand_edit_and_a_changed_frozen_table() {
    let Some(db) = Db::create().await else { return };
    db.freeze("both").await;
    let row = blade(&db).await;
    db.ext_row("t1", "defs.items", "30071", "change", row).await;
    run(&db.pool, format!("INSERT INTO {}.ext_ranges (extension, kind, major, minor_low, minor_high) VALUES ('t1', 'item', 30, 70, 72)", db.s.content)).await;
    db.editor.content_rebuild(&[Side::Defs, Side::World]).await.unwrap();
    assert_eq!(db.editor.content_check().await.unwrap(), Vec::<String>::new());

    run(&db.pool, format!("UPDATE {}.items SET itemname = 'Hacked' WHERE itemid = 30071", db.s.defs)).await;
    run(&db.pool, format!("DELETE FROM {}.mob_items WHERE mobid = 2133 AND itemorder = 3", db.s.defs)).await;
    let findings = db.editor.content_check().await.unwrap();
    let has = |needle: &str| findings.iter().any(|f| f.contains(needle));
    assert!(has("defs.items 30071: hand edit? itemname live 'Hacked', expected 'Blade'"), "{findings:#?}");
    assert!(has("defs.mob_items") && has("not in the live table"), "{findings:#?}");
    assert_eq!(findings.len(), 2, "{findings:#?}");

    db.editor.content_rebuild(&[Side::Defs]).await.unwrap();
    run(&db.pool, format!("UPDATE {}.mobs SET hp = 7 WHERE mobid = 10", db.s.original_defs)).await;
    let findings = db.editor.content_check().await.unwrap();
    assert!(findings.iter().any(|f| f.starts_with(&format!("{}.mobs: the frozen copy changed since the import", db.s.original_defs))), "{findings:#?}");
    db.drop().await;
}

#[tokio::test]
async fn check_finds_ids_outside_the_ranges_and_settings_two_enabled_extensions_share() {
    let Some(db) = Db::create().await else { return };
    db.freeze("both").await;
    let row = blade(&db).await;
    db.ext_row("t1", "defs.items", "30071", "change", row).await;
    run(&db.pool, format!("INSERT INTO {c}.ext_ranges (extension, kind, major, minor_low, minor_high) VALUES ('t1', 'item', 30, 1, 10)", c = db.s.content)).await;
    run(&db.pool, format!("INSERT INTO {c}.extensions (name, version, title, description, manifest) VALUES ('t2', '1.0.0', 't', 'd', '{{}}')", c = db.s.content)).await;
    run(&db.pool, format!("INSERT INTO {c}.ext_settings (extension, name, value) VALUES ('t1', 'kill_boost_skills', 'a'), ('t2', 'kill_boost_skills', 'b')", c = db.s.content)).await;
    db.editor.content_rebuild(&[Side::Defs, Side::World]).await.unwrap();
    let findings = db.editor.content_check().await.unwrap();
    assert!(findings.iter().any(|f| f == "defs.items 30071: owned by t1, outside the ids it declares"), "{findings:#?}");
    assert!(findings.iter().any(|f| f == "setting kill_boost_skills: set by the enabled extensions t1, t2"), "{findings:#?}");
    run(&db.pool, format!("UPDATE {}.extensions SET enabled = false WHERE name = 't2'", db.s.content)).await;
    run(&db.pool, format!("INSERT INTO {}.ext_ranges (extension, kind, major, minor_low, minor_high) VALUES ('t1', 'item', 30, 71, 71)", db.s.content)).await;
    assert_eq!(db.editor.content_check().await.unwrap(), Vec::<String>::new(), "disabled extensions do not clash; an id in its range is fine");
    db.drop().await;
}

#[tokio::test]
async fn origin_prints_the_original_the_extension_and_the_edits() {
    let Some(db) = Db::create().await else { return };
    db.freeze("both").await;
    let row = blade(&db).await;
    db.ext_row("t1", "defs.items", "30071", "change", row).await;
    db.draft(set("item_grade", "30071", "grade", json!(77))).await;
    db.apply("raise the grade").await;
    db.editor.content_rebuild(&[Side::Defs]).await.unwrap();
    let lines = db.editor.content_origin("items", "30071").await.unwrap();
    let text = lines.join("\n");
    assert!(text.contains("defs.items 30071"), "{text}");
    assert!(text.contains(r#""itemname":"Sword""#) && text.contains("original ("), "{text}");
    assert!(text.contains("extension t1 (change): itemname = 'Blade'"), "{text}");
    assert!(text.contains("edit item_grade grade = 77") && text.contains("change set #"), "{text}");
    assert!(text.contains("live against the original: grade: 5 -> 77; itemname: 'Sword' -> 'Blade'") || text.contains("live against the original: itemname: 'Sword' -> 'Blade'; grade: 5 -> 77"), "{text}");
    let plain = db.editor.content_origin("defs.mobs", "10").await;
    assert!(plain.is_err() || plain.unwrap().iter().any(|l| l.contains("none owns this row")));
    let message = refusal(db.editor.content_origin("items", "30/71").await.unwrap_err());
    assert!(message.contains("key of 1 parts"), "{message}");
    db.drop().await;
}

#[tokio::test]
async fn status_without_imports_names_setup_and_with_them_the_state() {
    let Some(db) = Db::create().await else { return };
    let lines = db.editor.content_status().await.unwrap();
    assert!(lines[0].contains("no original data is recorded as imported") && lines[0].contains("tools/setup.sh"), "{lines:#?}");
    assert!(lines.iter().any(|l| l == "extensions: none installed") && lines.iter().any(|l| l == "no client patch is due"), "{lines:#?}");

    db.freeze("both").await;
    let row = blade(&db).await;
    db.ext_row("t1", "defs.items", "30071", "change", row).await;
    let lines = db.editor.content_status().await.unwrap();
    assert!(lines.iter().any(|l| l.starts_with("defs: imported ") && l.contains("rebuild DUE (never run)")), "{lines:#?}");
    db.editor.content_rebuild(&[Side::Defs]).await.unwrap();
    let lines = db.editor.content_status().await.unwrap();
    assert!(lines.iter().any(|l| l.starts_with("defs: imported ") && l.contains("; rebuilt ")), "{lines:#?}");
    assert!(lines.iter().any(|l| l == "extension t1 1.0.0: enabled, 1 rows, 0 client rows, 0 settings"), "{lines:#?}");
    run(&db.pool, format!("INSERT INTO {c}.client_items (type, typeid, extension, name, description, note) VALUES (30, 71, 't1', 'a', 'b', 'n')", c = db.s.content)).await;
    let lines = db.editor.content_status().await.unwrap();
    assert!(lines.iter().any(|l| l.starts_with("a client patch is due: 1 client rows")), "{lines:#?}");
    db.drop().await;
}
