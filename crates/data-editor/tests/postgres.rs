//! Runs the editor against a real PostgreSQL inside throwaway schemas (see `common`). Skipped (and says so) when DATABASE_URL is not set.

mod common;

use common::*;
use serde_json::{Value, json};
use sqlx::AssertSqlSafe;

#[tokio::test]
async fn draft_apply_writes_the_tables_the_edits_and_the_log() {
    let Some(db) = Db::create().await else { return };
    db.draft(set("mob_drop", "2133/1", "droprate", json!(50))).await;
    db.draft(set("mob", "2133", "hp", json!(6000))).await;
    db.draft(set("item_grade", "30071", "grade", json!(77))).await;
    assert_eq!(db.editor.draft_list().await.unwrap().len(), 3);
    let applied = db.apply("tune the boss").await;
    assert_eq!(applied.changes, 3);
    let effects: Vec<_> = applied.effects.iter().map(|e| e.name()).collect();
    assert_eq!(effects, ["live", "next spawn"]);

    assert_eq!(db.value("mob_items", "droprate", "mobid = 2133 AND itemorder = 1").await, 50);
    assert_eq!(db.value("mobs", "hp", "mobid = 2133").await, 6000);
    assert_eq!(db.value("items", "grade", "itemid = 30071").await, 77);
    assert!(db.editor.draft_list().await.unwrap().is_empty(), "the draft is empty after an Apply");

    let fields = db.scalar(format!("SELECT jsonb_agg(jsonb_build_object('t', target, 'k', key, 'f', field, 'v', value, 'o', original, 'cs', change_set) ORDER BY target) AS v FROM {}.fields", db.s.edits)).await;
    assert_eq!(fields["v"][0], json!({"t": "item_grade", "k": "30071", "f": "grade", "v": 77, "o": 5, "cs": applied.change_set}));
    assert_eq!(fields["v"][1], json!({"t": "mob", "k": "2133", "f": "hp", "v": 6000, "o": 5000, "cs": applied.change_set}));
    assert_eq!(fields["v"][2], json!({"t": "mob_drop", "k": "2133/1", "f": "droprate", "v": 50, "o": 5, "cs": applied.change_set}));

    let log = db.editor.history_detail(applied.change_set).await.unwrap();
    assert_eq!(log.len(), 3);
    assert_eq!(
        (log[0].target.as_str(), log[0].action.as_str(), log[0].before.clone(), log[0].after.clone()),
        ("mob_drop", "set", Some(json!(5)), Some(json!(50)))
    );
    let history = db.editor.history(10).await.unwrap();
    assert_eq!((history[0].message.as_str(), history[0].made_by.as_str(), history[0].changes), ("tune the boss", "tester", 3));

    // Setting a field back to its original value removes the edit and logs an unset.
    db.draft(set("mob", "2133", "hp", json!(5000))).await;
    let second = db.apply("back again").await;
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.fields WHERE target = 'mob'", db.s.edits)).await, 0);
    let log = db.editor.history_detail(second.change_set).await.unwrap();
    assert_eq!((log[0].action.as_str(), log[0].before.clone(), log[0].after.clone()), ("unset", Some(json!(6000)), Some(json!(5000))));
    assert_eq!(db.value("mobs", "hp", "mobid = 2133").await, 5000);
    db.drop().await;
}

#[tokio::test]
async fn bad_lines_never_reach_the_draft() {
    let Some(db) = Db::create().await else { return };
    let cases = [
        (set("mob_drop", "2133/1", "droprate", json!(101)), "0 to 100"),
        (set("mob_drop", "2133/7", "droprate", json!(1_000_001)), "per million"),
        (set("mob_drop", "2133/0", "droprate", json!(1)), "entries 1 to 9"),
        (set("mob", "2133", "mobname", json!("x")), "Monster.SData"),
        (set("mob", "2133", "hp", json!(0)), "from 1 to"),
        (set("mob", "9999", "hp", json!(5)), "does not exist"),
        (set("item_grade", "30071", "buy", json!(5)), "cannot be edited"),
        (set("spawn_area", "2/57", "x2", json!(5000)), "inside map 2"),
        (set("spawn_area", "2/57", "x2", json!(5)), "x1 must be less than x2"),
        (set("portal", "2/1", "country", json!(1)), "instance portal"),
        (set("portal", "2/0", "to_map", json!(99)), "does not exist"),
        (set("portal", "2/0", "pos_x", json!(5)), "cannot be edited"),
        (set("boss_group", "1", "random_min", json!(5)), "not set on this row"),
        (set("boss_group", "1", "mobs", json!([10])), "length"),
        (set("obelisk", "1", "mob_light", json!(12)), "cannot spawn"),
        (add("spawn_area", "2/99", area_fields(60.0)), "from 100000"),
        (add("spawn_area", "2/57", area_fields(60.0)), "from 100000"),
        (add("spawn_area", "2/100000", json!({"x1": 5})), "needs the field"),
        (add("spawn_mob", "2/57/1", json!({"mob_id": 10, "count": 1})), "from 1000"),
        (add("spawn_mob", "2/57/1000", json!({"mob_id": 10, "count": 1})), "exists already"),
        (add("spawn_mob", "2/59/1000", json!({"mob_id": 10, "count": 1})), "area 2/59 does not exist"),
        (add("portal", "2/9", json!({})), "cannot be added"),
        (remove("portal", "2/0"), "cannot be removed"),
        (remove("spawn_area", "2/77"), "does not exist"),
    ];
    for (line, expect) in cases {
        let error = refusal(db.editor.draft_add(&line, "tester").await.unwrap_err());
        assert!(error.contains(expect), "{line:?}: {error}");
    }
    assert!(db.editor.draft_list().await.unwrap().is_empty());
    assert!(refusal(db.editor.apply_draft("x", "tester").await.unwrap_err()).contains("empty"));
    db.draft(set("mob", "10", "exp", json!(7))).await;
    assert!(refusal(db.editor.apply_draft("  ", "tester").await.unwrap_err()).contains("message"));

    // A removal is refused while the draft still changes that row; a line for a row the draft removed is refused.
    db.draft(set("spawn_mob", "2/57/1000", "count", json!(4))).await;
    assert!(refusal(db.editor.draft_add(&remove("spawn_area", "2/57"), "t").await.unwrap_err()).contains("pending change"));
    db.draft(remove("spawn_area", "2/58")).await;
    assert!(db.editor.draft_add(&set("spawn_area", "2/58", "x1", json!(31)), "t").await.is_err());
    db.drop().await;
}

#[tokio::test]
async fn undo_reverses_a_change_set_and_is_refused_after_a_later_change() {
    let Some(db) = Db::create().await else { return };
    db.draft(set("mob", "10", "hp", json!(150))).await;
    let a = db.apply("a").await;
    db.draft(set("mob", "10", "hp", json!(175))).await;
    let b = db.apply("b").await;
    let error = refusal(db.editor.undo(a.change_set, "tester").await.unwrap_err());
    assert!(error.contains("mob 10 hp") && error.contains(&format!("#{}", b.change_set)), "{error}");
    assert_eq!(db.value("mobs", "hp", "mobid = 10").await, 175);

    let undo_b = db.editor.undo(b.change_set, "tester").await.unwrap();
    assert_eq!(db.value("mobs", "hp", "mobid = 10").await, 150);
    let history = db.editor.history(10).await.unwrap();
    assert_eq!(history[0].message, format!("Undo #{}: b", b.change_set));
    assert_eq!(history[0].undoes, Some(b.change_set));
    assert_eq!(history.iter().find(|h| h.id == b.change_set).unwrap().undone_by, Some(undo_b.change_set));
    assert!(refusal(db.editor.undo(b.change_set, "tester").await.unwrap_err()).contains("undone already"));

    // With b undone, a can be undone: the original value comes back and the edit is gone.
    db.editor.undo(a.change_set, "tester").await.unwrap();
    assert_eq!(db.value("mobs", "hp", "mobid = 10").await, 100);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.fields", db.s.edits)).await, 0);
    // Undoing the undo of b puts b's value back (and a is undone first, so it needs that undo undone... which is refused).
    assert!(db.editor.undo(undo_b.change_set, "tester").await.is_err());
    assert!(refusal(db.editor.undo(999, "tester").await.unwrap_err()).contains("no change set"));
    db.drop().await;
}

#[tokio::test]
async fn spawn_areas_can_be_added_removed_and_brought_back() {
    let Some(db) = Db::create().await else { return };
    db.draft(add("spawn_area", "2/100000", area_fields(500.0))).await;
    db.draft(add("spawn_mob", "2/100000/1", json!({"mob_id": 11, "count": 4}))).await;
    db.draft(set("spawn_mob", "2/57/1000", "count", json!(9))).await;
    let first = db.apply("a new area").await;
    assert_eq!(first.effects.iter().map(|e| e.name()).collect::<Vec<_>>(), ["restart"]);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.mob_areas WHERE map = 2", db.s.world)).await, 3);
    assert_eq!(db.count(format!("SELECT count(*) FROM {w}.mob_area_mobs m JOIN {w}.mob_areas a ON a.id = m.mob_area_id WHERE a.seq = 100000 AND m.mob_id = 11 AND m.count = 4", w = db.s.world)).await, 1);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows WHERE action = 'add'", db.s.edits)).await, 2);

    // Removing the original area takes its monsters (and the edit on one of them) with it; the removal keeps them as a snapshot.
    db.draft(remove("spawn_area", "2/57")).await;
    let removal = db.apply("drop an area").await;
    assert_eq!(db.count(format!("SELECT count(*) FROM {w}.mob_areas WHERE seq = 57", w = db.s.world)).await, 0);
    assert_eq!(
        db.count(format!("SELECT count(*) FROM {w}.mob_area_mobs", w = db.s.world)).await,
        1,
        "only the added area's monster is left"
    );
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.fields WHERE target = 'spawn_mob' AND key LIKE '2/57/%'", db.s.edits)).await, 0);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows WHERE action = 'remove'", db.s.edits)).await, 1);

    // Undo brings the area, its monsters and the edit back.
    db.editor.undo(removal.change_set, "tester").await.unwrap();
    assert_eq!(db.count(format!("SELECT count(*) FROM {w}.mob_areas WHERE seq = 57", w = db.s.world)).await, 1);
    assert_eq!(
        db.count(format!(
            "SELECT count(*) FROM {w}.mob_area_mobs m JOIN {w}.mob_areas a ON a.id = m.mob_area_id WHERE a.seq = 57",
            w = db.s.world
        ))
        .await,
        2
    );
    assert_eq!(db.value("mob_area_mobs", "count", "seq = 1000").await, 9);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.fields WHERE key = '2/57/1000' AND original = '3'::jsonb", db.s.edits)).await, 1);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows WHERE action = 'remove'", db.s.edits)).await, 0);

    // Removing an added area leaves no trace in the edits; undoing that brings the area back as an added one.
    db.draft(remove("spawn_area", "2/100000")).await;
    let gone = db.apply("never mind").await;
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows", db.s.edits)).await, 0);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.fields WHERE key LIKE '2/100000%'", db.s.edits)).await, 0);
    db.editor.undo(gone.change_set, "tester").await.unwrap();
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows WHERE action = 'add'", db.s.edits)).await, 2);
    assert_eq!(db.count(format!("SELECT count(*) FROM {w}.mob_areas WHERE seq = 100000", w = db.s.world)).await, 1);

    // An edit to the new area's own field, then undo of just that, works; with it in force, undoing the change set that added the area
    // is refused (the edit touched the area), and once it is undone the first change set can be undone.
    db.draft(set("spawn_area", "2/100000", "x2", json!(520))).await;
    let edit = db.apply("wider").await;
    assert_eq!(db.value("mob_areas", "x2", "seq = 100000").await, 520);
    assert!(refusal(db.editor.undo(first.change_set, "tester").await.unwrap_err()).contains("spawn_area 2/100000"));
    db.editor.undo(edit.change_set, "tester").await.unwrap();
    assert_eq!(db.value("mob_areas", "x2", "seq = 100000").await, 510);
    db.editor.undo(first.change_set, "tester").await.unwrap();
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.mob_areas WHERE map = 2", db.s.world)).await, 2);
    assert_eq!(db.value("mob_area_mobs", "count", "seq = 1000").await, 3);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.fields", db.s.edits)).await, 0);
    assert_eq!(db.count(format!("SELECT count(*) FROM {}.rows", db.s.edits)).await, 0);
    db.drop().await;
}

#[tokio::test]
async fn a_fresh_load_gets_the_edits_back_and_the_differences_reported() {
    let Some(db) = Db::create().await else { return };
    db.draft(set("mob", "10", "hp", json!(150))).await;
    db.draft(set("mob", "11", "money1", json!(9))).await;
    db.draft(add("spawn_area", "2/100000", area_fields(500.0))).await;
    db.draft(add("spawn_mob", "2/100000/1", json!({"mob_id": 10, "count": 4}))).await;
    db.draft(remove("spawn_area", "2/57")).await;
    db.draft(set("portal", "2/0", "to_map", json!(2))).await;
    db.draft(set("boss_group", "1", "mobs", json!([2133, 10]))).await;
    db.apply("many edits").await;

    // The Apply made the markers (writing the then empty edits into the fresh tables first); a second apply has nothing to write.
    let again = db.editor.apply_edits().await.unwrap();
    assert!(again.fresh.is_empty() && again.written == 0 && again.problems.is_empty(), "{again:?}");
    let status = db.editor.status().await.unwrap();
    assert!(status.defs_marker.is_some() && status.world_marker.is_some());
    assert_eq!((status.field_edits, status.added_rows, status.removed_rows), (12, 2, 1));

    // A hand edit of an edited field is repaired.
    run(&db.pool, format!("UPDATE {}.mobs SET hp = 1 WHERE mobid = 10", db.s.defs)).await;
    let repair = db.editor.apply_edits().await.unwrap();
    assert_eq!(repair.written, 1);
    assert_eq!(db.value("mobs", "hp", "mobid = 10").await, 150);

    // A fresh load: monster 10's HP changed to 120 under the edit, monster 11 is gone, area 2/100000 exists in the files now.
    db.load(Files { changed: true }).await;
    let report = db.editor.apply_edits().await.unwrap();
    assert_eq!(report.fresh.len(), 2);
    let texts: Vec<String> =
        report.problems.iter().map(|p| format!("{} {} {}: {}", p.target, p.key, p.field.clone().unwrap_or_default(), p.text)).collect();
    let has = |needle: &str| texts.iter().any(|t| t.contains(needle));
    assert!(has("mob 10 hp: the import changed hp 100 -> 120 under your edit to 150"), "{texts:#?}");
    assert!(has("mob 11 money1: the row does not exist in the new files"), "{texts:#?}");
    assert!(has("spawn_area 2/100000 : the import has a row with this key now"), "{texts:#?}");
    assert!(report.problems.iter().all(|p| p.change_set.is_some()));
    // The edits that could be written were: the new HP, the removal, the portal; the colliding area was left alone.
    assert_eq!(db.value("mobs", "hp", "mobid = 10").await, 150);
    assert_eq!(db.count(format!("SELECT count(*) FROM {w}.mob_areas WHERE seq = 57", w = db.s.world)).await, 0);
    assert_eq!(db.value("map_portals", "to_map", "map = 2 AND seq = 0").await, 2);
    assert_eq!(db.value("boss_groups", "mobs", "seq = 1").await, json!([2133, 10]));
    assert_eq!(db.value("mob_areas", "x1", "seq = 100000").await, 500, "the imported area stays as imported");
    assert_eq!(
        db.count(format!(
            "SELECT count(*) FROM {w}.mob_area_mobs m JOIN {w}.mob_areas a ON a.id = m.mob_area_id WHERE a.seq = 100000",
            w = db.s.world
        ))
        .await,
        0,
        "the colliding area's monsters were not added"
    );
    let original: Value = sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT original FROM {}.fields WHERE target = 'mob' AND key = '10' AND field = 'hp'",
        db.s.edits
    )))
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(original, 120, "the original is refreshed from the new files");

    // The next apply is quiet about the original (it was refreshed) and still tells which rows are gone.
    let next = db.editor.apply_edits().await.unwrap();
    assert!(next.fresh.is_empty());
    assert!(
        next.problems
            .iter()
            .all(|p| p.text.contains("does not exist any more") || p.text.contains("at the last load") || p.text.contains("was not added")),
        "{:?}",
        next.problems
    );
    assert_eq!(
        db.count(format!(
            "SELECT count(*) FROM {w}.mob_area_mobs m JOIN {w}.mob_areas a ON a.id = m.mob_area_id WHERE a.seq = 100000",
            w = db.s.world
        ))
        .await,
        0,
        "still not added to the imported area"
    );
    db.drop().await;
}

#[tokio::test]
async fn export_then_import_replace_gives_the_same_export() {
    let Some(db) = Db::create().await else { return };
    db.draft(set("mob", "10", "hp", json!(150))).await;
    db.draft(set("mob_drop", "2133/2", "grade", json!(12))).await;
    db.draft(add("spawn_area", "2/100000", area_fields(500.0))).await;
    db.draft(add("spawn_mob", "2/100000/1", json!({"mob_id": 10, "count": 4}))).await;
    db.draft(remove("spawn_area", "2/58")).await;
    db.apply("first").await;
    db.draft(set("portal", "2/0", "country", json!(2))).await;
    db.draft(set("obelisk_guard", "1/1", "count", json!(5))).await;
    let second = db.apply("second").await;
    db.editor.undo(second.change_set, "tester").await.unwrap();
    run(&db.pool, format!("INSERT INTO {}.map_names (map, name) VALUES (2, 'Cantabilian')", db.s.edits)).await;
    run(&db.pool, format!("INSERT INTO {}.map_images (map, content_type, image, x1, z1, x2, z2, source) VALUES (2, 'image/png', '\\x89504e470d0a', 0, 0, 1024, 1024, 'upload')", db.s.edits)).await;
    db.editor.apply_edits().await.unwrap();

    let plain = db.editor.export(false).await.unwrap();
    let with_images = db.editor.export(true).await.unwrap();
    assert_eq!(plain["format"], "shaiya-afterglow-edits");
    assert_eq!(plain["map_images"], json!([]));
    assert_eq!(with_images["map_images"][0]["image"], "iVBORw0K", "base64 without line breaks");
    assert_eq!(db.editor.export(false).await.unwrap(), plain, "two exports of the same edits are identical");
    assert_eq!(plain["change_sets"].as_array().unwrap().len(), 3);
    assert_eq!(plain["exported_at"], plain["change_sets"][2]["made_at"]);

    let tables_before =
        (db.value("mobs", "hp", "mobid = 10").await, db.count(format!("SELECT count(*) FROM {}.mob_areas", db.s.world)).await);

    assert!(refusal(db.editor.import(&with_images, false).await.unwrap_err()).contains("not empty"));
    let report = db.editor.import(&with_images, true).await.unwrap();
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert_eq!(db.editor.export(true).await.unwrap(), with_images, "import --replace then export is the same file");
    assert_eq!(
        (db.value("mobs", "hp", "mobid = 10").await, db.count(format!("SELECT count(*) FROM {}.mob_areas", db.s.world)).await),
        tables_before
    );

    // Undo still works on imported history (the identity continues after the imported ids).
    db.draft(set("mob", "10", "exp", json!(8))).await;
    let next = db.apply("after import").await;
    assert_eq!(next.change_set, 4);

    // Into empty edits, onto a database that never had them: another server's copy.
    let other = Db::create().await.unwrap();
    other.editor.import(&with_images, false).await.unwrap();
    assert_eq!(other.value("mobs", "hp", "mobid = 10").await, 150);
    other.drop().await;
    db.drop().await;
}
