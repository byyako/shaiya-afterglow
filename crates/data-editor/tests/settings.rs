//! The data editor's `setting` target (`docs/plans/ext-1-foundation.md` section 7.4) against a real PostgreSQL in throwaway schemas:
//! draft, apply, the game server's own `resolve` over the rows, undo, remove, export and import. Skipped without DATABASE_URL.

mod common;

use common::*;
use game_server::config::{FileLayer, Layer, Sources, Startup, read_file, resolve};
use serde_json::json;

fn startup() -> Startup {
    Startup { bind: "0.0.0.0:30810".parse().unwrap(), database_url: "postgres://localhost/test".into(), server_id: 1, config: None }
}

/// What `Config::load` reads from the database for the editor's layer.
async fn overrides(db: &Db) -> Vec<(String, String, Option<i32>)> {
    let (c, e) = (&db.s.content, &db.s.edits);
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT v.name, v.value, (SELECT max(f.change_set) FROM {e}.fields f WHERE f.target = 'setting' AND f.key = v.name) FROM {c}.setting_values v ORDER BY v.name"
    )))
    .fetch_all(&db.pool)
    .await
    .unwrap()
}

fn file(text: &str) -> Option<FileLayer> {
    Some(read_file("config/game.toml", text).unwrap())
}

#[tokio::test]
async fn a_setting_is_drafted_applied_and_taken_over_the_file_by_resolve() {
    let Some(db) = Db::create().await else { return };
    // `set` on a setting with no override makes the row (the dry run of the plan's section 9).
    db.draft(set("setting", "trade_chat_min_level", "value", json!(12))).await;
    db.draft(add("setting", "market_maps", json!({"value": [35, 36]}))).await;
    let applied = db.apply("tune the chat").await;
    assert_eq!(applied.changes, 2);
    assert_eq!(applied.effects.iter().map(|e| e.name()).collect::<Vec<_>>(), ["restart"]);
    assert_eq!(db.value_text(&format!("{}.setting_values", db.s.content), "value", "name = 'trade_chat_min_level'").await, "12");
    assert_eq!(db.value_text(&format!("{}.setting_values", db.s.content), "value", "name = 'market_maps'").await, "35,36");
    let fields = db.scalar(format!("SELECT jsonb_agg(jsonb_build_object('k', key, 'f', field, 'v', value, 'o', original) ORDER BY key) AS v FROM {}.fields", db.s.edits)).await;
    assert_eq!(fields["v"][1], json!({"k": "trade_chat_min_level", "f": "value", "v": "12", "o": null}), "an added override has no original");
    let rows = db.scalar(format!("SELECT jsonb_agg(jsonb_build_object('k', key, 'a', action) ORDER BY key) AS v FROM {}.rows", db.s.edits)).await;
    assert_eq!(rows["v"], json!([{"k": "market_maps", "a": "add"}, {"k": "trade_chat_min_level", "a": "add"}]));
    let log = db.editor.history_detail(applied.change_set).await.unwrap();
    assert_eq!(log.iter().map(|l| (l.target.as_str(), l.key.as_str(), l.action.as_str())).collect::<Vec<_>>(), [("setting", "trade_chat_min_level", "add"), ("setting", "market_maps", "add")]);

    // The game server's own resolve over those rows: the editor beats the file, which beats the default.
    let sources = Sources { file: file("trade_chat_min_level = 8\nguild_party_size = 2\n"), editor: overrides(&db).await, ..Sources::default() };
    let (config, all) = resolve(&startup(), &sources).unwrap();
    let layer = |name: &str| all.iter().find(|r| r.name == name).unwrap().layer.clone();
    assert_eq!(config.trade_chat_min_level, 12);
    assert_eq!(layer("trade_chat_min_level"), Layer::Editor(Some(applied.change_set)));
    assert_eq!((config.market_maps.clone(), config.guild_party_size), (vec![35, 36], 2));
    assert_eq!(layer("guild_party_size"), Layer::File);

    // Setting it again changes the row and keeps an add's lack of original; undo goes back step by step and ends with no override.
    db.draft(set("setting", "trade_chat_min_level", "value", json!("15"))).await;
    let second = db.apply("again").await;
    assert_eq!(overrides(&db).await[1].1, "15");
    db.editor.undo(second.change_set, "tester").await.unwrap();
    assert_eq!(overrides(&db).await[1].1, "12");
    db.editor.undo(applied.change_set, "tester").await.unwrap();
    assert!(overrides(&db).await.is_empty(), "undo removes the override: the layers below show again");
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.fields", db.s.edits)).await, 0);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows", db.s.edits)).await, 0);
    let (config, all) = resolve(&startup(), &Sources::default()).unwrap();
    assert_eq!((config.trade_chat_min_level, all.iter().find(|r| r.name == "trade_chat_min_level").unwrap().layer.clone()), (10, Layer::Default));
    db.drop().await;
}

#[tokio::test]
async fn bad_setting_lines_never_reach_the_draft() {
    let Some(db) = Db::create().await else { return };
    let cases = [
        (set("setting", "trade_chat_minlevel", "value", json!(12)), "is not a game setting"),
        (set("setting", "database_url", "value", json!("x")), "is not a game setting"),
        (set("setting", "ext.welcome.welcome_text", "value", json!("hi")), "the extension welcome is not installed"),
        (add("setting", "ext.welcome.welcome_text", json!({"value": "hi"})), "the extension welcome is not installed"),
        (add("setting", "ext.Welcome", json!({"value": "hi"})), "is not an extension option"),
        (set("setting", "trade_chat_min_level", "value", json!("ten")), "not a whole number"),
        (set("setting", "slot_payout_percent", "value", json!(0)), "1 to 1000"),
        (set("setting", "kill_boost_skills", "value", json!("200/1:101")), "times 101"),
        (set("setting", "market_maps", "value", json!({"a": 1})), "string, a number"),
        (set("setting", "market_maps", "value", json!([[1]])), "integers or strings"),
        (set("setting", "market_maps", "other", json!("1")), "one field, value"),
        (add("setting", "market_maps", json!({"value": "1", "other": 2})), "one field, value"),
        (add("setting", "market_maps", json!({})), "needs the field value"),
        (add("setting", "market_maps", json!("1")), "needs an object"),
        (remove("setting", "market_maps"), "no override"),
        (unset("setting", "market_maps", "value"), "no edit to undo"),
    ];
    for (line, expect) in cases {
        let error = refusal(db.editor.draft_add(&line, "tester").await.unwrap_err());
        assert!(error.contains(expect), "{line:?}: {error}");
    }
    assert!(db.editor.draft_list().await.unwrap().is_empty());
    // An override that exists is not added twice.
    db.draft(add("setting", "market_maps", json!({"value": "1"}))).await;
    db.apply("one").await;
    assert!(refusal(db.editor.draft_add(&add("setting", "market_maps", json!({"value": "2"})), "t").await.unwrap_err()).contains("exists already"));
    db.drop().await;
}

#[tokio::test]
async fn removing_an_override_falls_back_and_undo_brings_it_back() {
    let Some(db) = Db::create().await else { return };
    // A row that did not come through the editor (written by hand) is removed with a snapshot Undo restores.
    run(&db.pool, format!("INSERT INTO {}.setting_values (name, value) VALUES ('guild_party_size', '3')", db.s.content)).await;
    db.draft(set("setting", "guild_party_size", "value", json!(4))).await;
    let changed = db.apply("four").await;
    let fields = db.scalar(format!("SELECT value, original FROM {}.fields WHERE target = 'setting'", db.s.edits)).await;
    assert_eq!((fields["value"].clone(), fields["original"].clone()), (json!("4"), json!("3")), "a hand-made row's value is the original");
    db.draft(set("setting", "guild_party_size", "value", json!(3))).await;
    let back = db.apply("back to the original").await;
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.fields", db.s.edits)).await, 0, "the original again: the edit is gone");
    assert_eq!(db.editor.history_detail(back.change_set).await.unwrap()[0].action, "unset");
    db.editor.undo(back.change_set, "tester").await.unwrap();
    assert_eq!(overrides(&db).await[0].1, "4");
    db.editor.undo(changed.change_set, "tester").await.unwrap();

    db.draft(remove("setting", "guild_party_size")).await;
    let removed = db.apply("drop the override").await;
    assert!(overrides(&db).await.is_empty());
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows WHERE action = 'remove'", db.s.edits)).await, 1);
    // `apply` keeps it removed.
    db.editor.apply_edits().await.unwrap();
    assert!(overrides(&db).await.is_empty());
    db.editor.undo(removed.change_set, "tester").await.unwrap();
    assert_eq!(overrides(&db).await[0].1, "3");
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows", db.s.edits)).await, 0);
    db.drop().await;
}

#[tokio::test]
async fn export_and_import_carry_the_overrides() {
    let Some(db) = Db::create().await else { return };
    db.draft(set("setting", "exp_rate", "value", json!(2.5))).await;
    db.draft(set("setting", "card_win_chances", "value", json!([420, 200, 130, 100, 80]))).await;
    db.draft(set("mob", "10", "hp", json!(150))).await;
    db.apply("settings and a monster").await;
    let export = db.editor.export(false).await.unwrap();
    assert_eq!(export["fields"].as_array().unwrap().iter().filter(|f| f["target"] == "setting").count(), 2);

    // Into the same database with replace: reverted, then written again from the file; the same export comes out.
    let report = db.editor.import(&export, true).await.unwrap();
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert_eq!(db.editor.export(false).await.unwrap(), export);
    assert_eq!(overrides(&db).await.iter().map(|(n, v, _)| (n.as_str(), v.as_str())).collect::<Vec<_>>(), [("card_win_chances", "420,200,130,100,80"), ("exp_rate", "2.5")]);

    // Another server's copy: the rows are written by the import's apply.
    let other = Db::create().await.unwrap();
    other.editor.import(&export, false).await.unwrap();
    assert_eq!(overrides(&other).await.len(), 2);
    assert_eq!(other.value("mobs", "hp", "mobid = 10").await, 150);

    // An override the game server would not accept is reported and left alone.
    run(&other.pool, format!("DELETE FROM {}.setting_values", other.s.content)).await;
    run(&other.pool, format!("UPDATE {}.fields SET value = '\"-1\"' WHERE key = 'exp_rate'", other.s.edits)).await;
    let report = other.editor.apply_edits().await.unwrap();
    assert!(report.problems.iter().any(|p| p.target == "setting" && p.key == "exp_rate" && p.text.contains("left alone")), "{:?}", report.problems);
    assert_eq!(overrides(&other).await.len(), 1, "only the valid one was written");
    other.drop().await;
    db.drop().await;
}
