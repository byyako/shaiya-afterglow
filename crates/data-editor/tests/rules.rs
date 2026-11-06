//! The data editor's `rule` target (`docs/plans/ext-2-rules.md` section 4.5) against a real PostgreSQL in throwaway schemas: draft,
//! apply, the game server's own `resolve` over the rows, undo, remove, export and import. Skipped without DATABASE_URL.

mod common;

use common::*;
use game_server::config::{Sources, Startup};
use game_server::rules::{FileRule, RuleFile, RuleLayer, RuleSources, RuleState, resolve};
use serde_json::{Value, json};

fn config() -> game_server::config::Config {
    let startup =
        Startup { bind: "0.0.0.0:30810".parse().unwrap(), database_url: "postgres://localhost/test".into(), server_id: 1, config: None };
    game_server::config::resolve(&startup, &Sources::default()).unwrap().0
}

/// What the game server reads from the database for the editor's layer (`read_rule_layers_in`): `(name, rule, change set)`.
async fn rules_of(db: &Db) -> Vec<(String, Value, Option<i32>)> {
    let mut conn = db.pool.acquire().await.unwrap();
    game_server::config::read_rule_layers_in(&mut conn, &db.s.content, &db.s.edits).await.unwrap().editor
}

fn file_rule(name: &str, value: Value) -> FileRule {
    FileRule { name: name.into(), value, line: 7, key_lines: vec![] }
}

#[tokio::test]
async fn a_rule_is_drafted_applied_and_taken_over_the_files_by_resolve() {
    let Some(db) = Db::create().await else { return };
    db.draft(add("rule", "level-gap", json!({"rule": {"type": "level_gap", "gap": 15}}))).await;
    let applied = db.apply("a level gap").await;
    assert_eq!(applied.changes, 1);
    assert_eq!(applied.effects.iter().map(|e| e.name()).collect::<Vec<_>>(), ["live"]);
    let stored = db.scalar(format!("SELECT rule AS v FROM {}.rule_values WHERE name = 'level-gap'", db.s.content)).await;
    assert_eq!(stored["v"], json!({"type": "level_gap", "gap": 15}));
    let fields = db
        .scalar(format!(
            "SELECT jsonb_agg(jsonb_build_object('k', key, 'f', field, 'v', value, 'o', original)) AS v FROM {}.fields",
            db.s.edits
        ))
        .await;
    assert_eq!(
        fields["v"][0],
        json!({"k": "level-gap", "f": "rule", "v": {"type": "level_gap", "gap": 15}, "o": null}),
        "an added rule has no original"
    );
    let log = db.editor.history_detail(applied.change_set).await.unwrap();
    assert_eq!(
        log.iter().map(|l| (l.target.as_str(), l.key.as_str(), l.action.as_str())).collect::<Vec<_>>(),
        [("rule", "level-gap", "add")]
    );

    // The game server's own resolve over those rows: the editor's rule beats the file's of the same name.
    let sources = RuleSources {
        file: Some(RuleFile {
            path: "config/game.toml".into(),
            rules: vec![file_rule("level-gap", json!({"type": "level_gap", "gap": 10}))],
        }),
        editor: rules_of(&db).await,
        ..RuleSources::default()
    };
    let (set, all) = resolve(&config(), &sources).unwrap();
    let rule = all.iter().find(|r| r.name == "level-gap" && r.state == RuleState::InForce).unwrap();
    assert_eq!(rule.layer, RuleLayer::Editor(Some(applied.change_set)));
    assert_eq!(rule.value, json!({"type": "level_gap", "gap": 15}));
    assert!(
        all.iter().any(|r| r.name == "level-gap" && r.state == RuleState::Replaced(RuleLayer::Editor(Some(applied.change_set)))),
        "the file's is replaced"
    );
    assert_eq!(set.list()[0].layer, format!("data editor (change set {})", applied.change_set));

    // Setting the rule changes the row; undo goes back step by step and ends with no rule.
    db.draft(set_rule("level-gap", json!({"type": "level_gap", "gap": 25}))).await;
    let second = db.apply("wider").await;
    assert_eq!(rules_of(&db).await[0].1, json!({"type": "level_gap", "gap": 25}));
    let fields = db.scalar(format!("SELECT value AS v FROM {}.fields WHERE key = 'level-gap'", db.s.edits)).await;
    assert_eq!(fields["v"], json!({"type": "level_gap", "gap": 25}));
    db.editor.undo(second.change_set, "tester").await.unwrap();
    assert_eq!(rules_of(&db).await[0].1, json!({"type": "level_gap", "gap": 15}));
    db.editor.undo(applied.change_set, "tester").await.unwrap();
    assert!(rules_of(&db).await.is_empty(), "undo removes the rule: the layers below show again");
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.fields", db.s.edits)).await, 0);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows", db.s.edits)).await, 0);
    db.drop().await;
}

fn set_rule(name: &str, rule: Value) -> data_editor::store::Line {
    set("rule", name, "rule", rule)
}

#[tokio::test]
async fn bad_rule_lines_never_reach_the_draft() {
    let Some(db) = Db::create().await else { return };
    let ok = json!({"type": "level_gap", "gap": 15});
    let cases = [
        (add("rule", "Level Gap", json!({"rule": ok})), "is not a rule name"),
        (add("rule", "setting.market_maps", json!({"rule": ok})), "change the setting `market_maps` instead"),
        (add("rule", "ext.welcome.x", json!({"rule": ok})), "reserved for chunk 5"),
        (add("rule", "x", json!({"rule": {"type": "level_gap", "gap": 0}})), "rules.x.gap: 0 is not 1 to 255"),
        (add("rule", "x", json!({"rule": {"type": "level_gap", "gap": 5, "hour": 3}})), "unknown key \"hour\""),
        (add("rule", "x", json!({"rule": {"type": "nothing"}})), "unknown type"),
        (add("rule", "x", json!({"rule": {"maps": [1]}})), "a rule needs a type"),
        (add("rule", "x", json!({"rule": "level_gap"})), "an object of its keys"),
        (add("rule", "x", json!({"rule": ok, "other": 1})), "one field, rule"),
        (add("rule", "x", json!({})), "needs the field rule"),
        (add("rule", "x", json!("1")), "needs an object"),
        (set("rule", "x", "other", json!({})), "one field, rule"),
        (remove("rule", "x"), "no rule of this name"),
        (unset("rule", "x", "rule"), "no edit to undo"),
    ];
    for (line, expect) in cases {
        let error = refusal(db.editor.draft_add(&line, "tester").await.unwrap_err());
        assert!(error.contains(expect), "{line:?}: {error}");
    }
    assert!(db.editor.draft_list().await.unwrap().is_empty());
    db.draft(add("rule", "x", json!({"rule": ok}))).await;
    db.apply("one").await;
    assert!(refusal(db.editor.draft_add(&add("rule", "x", json!({"rule": ok})), "t").await.unwrap_err()).contains("exists already"));
    db.drop().await;
}

#[tokio::test]
async fn enabled_false_turns_an_extensions_rule_off_and_an_unmatched_one_is_an_error_at_resolve() {
    let Some(db) = Db::create().await else { return };
    db.draft(add("rule", "pets/no-capture", json!({"rule": {"enabled": false}}))).await;
    let applied = db.apply("turn it off").await;
    let sources = RuleSources {
        extensions: vec![("pets".into(), "no-capture".into(), json!({"type": "deny", "point": "attack", "maps": [2]}))],
        editor: rules_of(&db).await,
        ..RuleSources::default()
    };
    let (set, all) = resolve(&config(), &sources).unwrap();
    assert!(set.list().iter().all(|l| &*l.name != "pets/no-capture"));
    assert!(
        all.iter().any(|r| r.name == "pets/no-capture" && r.state == RuleState::TurnedOff(RuleLayer::Editor(Some(applied.change_set)))),
        "{all:?}"
    );
    // Without that extension the editor cannot know: the server's start (and `game-server rules`) says so.
    let alone = RuleSources { editor: rules_of(&db).await, ..RuleSources::default() };
    let message = format!("{:#}", resolve(&config(), &alone).unwrap_err());
    assert!(
        message.starts_with("data editor (change set")
            && message.contains("rules.pets/no-capture: no rule of that name in a lower layer to turn off"),
        "{message}"
    );
    db.drop().await;
}

#[tokio::test]
async fn removing_a_rule_falls_back_and_undo_brings_it_back() {
    let Some(db) = Db::create().await else { return };
    // A row written by hand is removed with a snapshot that Undo restores.
    run(&db.pool, format!("INSERT INTO {}.rule_values (name, rule) VALUES ('hand', '{{\"type\": \"exp\", \"factor\": 2}}')", db.s.content))
        .await;
    db.draft(set_rule("hand", json!({"type": "exp", "factor": 3}))).await;
    let changed = db.apply("three").await;
    let fields = db.scalar(format!("SELECT value, original FROM {}.fields WHERE target = 'rule'", db.s.edits)).await;
    assert_eq!(
        (fields["value"].clone(), fields["original"].clone()),
        (json!({"type": "exp", "factor": 3}), json!({"type": "exp", "factor": 2})),
        "a hand-made row's rule is the original"
    );
    db.draft(set_rule("hand", json!({"type": "exp", "factor": 2}))).await;
    let back = db.apply("back to the original").await;
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.fields", db.s.edits)).await, 0, "the original again: the edit is gone");
    assert_eq!(db.editor.history_detail(back.change_set).await.unwrap()[0].action, "unset");
    db.editor.undo(back.change_set, "tester").await.unwrap();
    assert_eq!(rules_of(&db).await[0].1, json!({"type": "exp", "factor": 3}));
    db.editor.undo(changed.change_set, "tester").await.unwrap();

    db.draft(remove("rule", "hand")).await;
    let removed = db.apply("drop it").await;
    assert!(rules_of(&db).await.is_empty());
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows WHERE action = 'remove'", db.s.edits)).await, 1);
    db.editor.apply_edits().await.unwrap();
    assert!(rules_of(&db).await.is_empty(), "apply keeps it removed");
    db.editor.undo(removed.change_set, "tester").await.unwrap();
    assert_eq!(rules_of(&db).await[0].1, json!({"type": "exp", "factor": 2}));
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows", db.s.edits)).await, 0);
    db.drop().await;
}

#[tokio::test]
async fn export_and_import_carry_the_rules() {
    let Some(db) = Db::create().await else { return };
    db.draft(add("rule", "level-gap", json!({"rule": {"type": "level_gap", "gap": 15}}))).await;
    db.draft(add("rule", "jumps", json!({"rule": {"type": "enchant", "jumps": [[1, 70], [2, 20], [3, 10]]}}))).await;
    db.draft(set("mob", "10", "hp", json!(150))).await;
    db.apply("rules and a monster").await;
    let export = db.editor.export(false).await.unwrap();
    assert_eq!(export["fields"].as_array().unwrap().iter().filter(|f| f["target"] == "rule").count(), 2);

    // Into the same database with replace: reverted, then written again from the file; the same export comes out.
    let report = db.editor.import(&export, true).await.unwrap();
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert_eq!(db.editor.export(false).await.unwrap(), export);
    assert_eq!(rules_of(&db).await.iter().map(|(n, _, _)| n.as_str()).collect::<Vec<_>>(), ["jumps", "level-gap"]);

    // Another server's copy: the rows are written by the import's apply.
    let other = Db::create().await.unwrap();
    other.editor.import(&export, false).await.unwrap();
    assert_eq!(rules_of(&other).await.len(), 2);
    assert_eq!(other.value("mobs", "hp", "mobid = 10").await, 150);

    // A rule the game server would not accept is reported and left alone.
    run(&other.pool, format!("DELETE FROM {}.rule_values", other.s.content)).await;
    run(
        &other.pool,
        format!("UPDATE {}.fields SET value = '{{\"type\": \"level_gap\", \"gap\": 0}}' WHERE key = 'level-gap'", other.s.edits),
    )
    .await;
    let report = other.editor.apply_edits().await.unwrap();
    assert!(
        report.problems.iter().any(|p| p.target == "rule" && p.key == "level-gap" && p.text.contains("left alone")),
        "{:?}",
        report.problems
    );
    assert_eq!(rules_of(&other).await.len(), 1, "only the valid one was written");
    other.drop().await;
    db.drop().await;
}
