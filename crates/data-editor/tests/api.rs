//! The page's API against a real PostgreSQL in throwaway schemas (see `common`), through the router itself with a loopback address.
//! One round trip per kind: maps, monsters and grades, the draft and Apply, history and Undo, pictures and names, the export.

mod common;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::Db;
use data_editor::serve::{App, router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

/// A 1×1 PNG.
const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, b'I', b'H', b'D', b'R', 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0, 0x90,
    0x77, 0x53, 0xDE, 0, 0, 0, 0x0C, b'I', b'D', b'A', b'T', 8, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0, 0, 3, 1, 1, 0, 0x18, 0xDD, 0x8D, 0xB0, 0,
    0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82,
];

struct Api {
    router: axum::Router,
}

impl Api {
    fn new(db: &Db) -> Self {
        Self { router: router(Arc::new(App { editor: db.editor.clone(), export_file: None, hosts: Vec::new() })) }
    }

    async fn raw(&self, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Vec<u8>, axum::http::HeaderMap) {
        let mut request = Request::builder().method(method.clone()).uri(path).header("host", "localhost:30890");
        if method != Method::GET {
            request = request.header("content-type", "application/json").header("x-editor", "1");
        }
        let mut request = request.body(Body::from(body.map(|b| b.to_string()).unwrap_or_default())).unwrap();
        request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))));
        let response = self.router.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        (parts.status, body.collect().await.unwrap().to_bytes().to_vec(), parts.headers)
    }

    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let (status, bytes, _) = self.raw(method, path, body).await;
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    async fn get(&self, path: &str) -> Value {
        let (status, v) = self.call(Method::GET, path, None).await;
        assert_eq!(status, StatusCode::OK, "GET {path}: {v}");
        v
    }

    async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.call(Method::POST, path, Some(body)).await
    }
}

#[tokio::test]
async fn maps_and_one_map_with_everything_on_it() {
    let Some(db) = Db::create().await else { return };
    let api = Api::new(&db);
    let maps = api.get("/api/maps").await;
    let two = maps.as_array().unwrap().iter().find(|m| m["id"] == 2).unwrap();
    assert_eq!(two["areas"], 2);
    assert_eq!(two["monsters"], 2, "monsters 10 and 2133 are spawned");
    assert_eq!(two["has_image"], false);

    common::run(&db.pool, format!("UPDATE {}.mobs SET attackspecial3 = 2 WHERE mobid = 10", db.s.defs)).await;
    let map = api.get("/api/maps/2").await;
    assert_eq!(map["map"]["size"], 1024);
    assert_eq!(map["areas"].as_array().unwrap().len(), 2);
    let area = &map["areas"][0];
    assert_eq!(area["seq"], 57);
    assert_eq!(area["mobs"].as_array().unwrap().len(), 2);
    let rat = map["monsters"].as_array().unwrap().iter().find(|m| m["mobid"] == 10).unwrap();
    assert_eq!((rat["name"].clone(), rat["here"].clone(), rat["areas"].clone()), (json!("Rat"), json!(3), json!(1)));
    assert_eq!(rat["respawn_secs"], 60, "code 2 is a minute");
    assert_eq!(map["portals"].as_array().unwrap().len(), 2);
    assert_eq!(map["obelisks"][0]["guards"].as_array().unwrap().len(), 1);
    assert_eq!(map["bosses"][0]["followers"].as_array().unwrap().len(), 1);
    assert_eq!(map["mob_info"]["2133"]["name"], "Boss");

    let (status, body) = api.call(Method::GET, "/api/maps/99", None).await;
    assert_eq!((status, body["error"].as_str().unwrap()), (StatusCode::NOT_FOUND, "there is no map 99"));
    let (status, bits, headers) = api.raw(Method::GET, "/api/maps/2/blocked", None).await;
    assert_eq!((status, headers["x-map-size"].to_str().unwrap(), bits.len()), (StatusCode::OK, "1024", 1));
    db.drop().await;
}

#[tokio::test]
async fn monsters_grades_items_and_the_one_item_helper() {
    let Some(db) = Db::create().await else { return };
    let api = Api::new(&db);
    let mob = api.get("/api/mobs/2133").await;
    assert_eq!(mob["def"]["mobname"], "Boss");
    assert!(mob["def"].get("rowid").is_none());
    let drops = mob["drops"].as_array().unwrap();
    assert_eq!(drops.len(), 9, "entries 1 to 9");
    // entry 1: grade 10, 5 %; entry 7: grade 70, 35 per 100 = 35 per million shown as 0.0035 %
    assert_eq!((drops[0]["grade"].clone(), drops[0]["percent"].clone()), (json!(10), json!(5.0)));
    assert!((drops[6]["percent"].as_f64().unwrap() - 0.0035).abs() < 1e-9);
    assert_eq!(drops[0]["items"], 0, "no item has grade 10 in the scratch data");
    assert_eq!(api.call(Method::GET, "/api/mobs/4242", None).await.0, StatusCode::NOT_FOUND);

    let grade = api.get("/api/grades/5").await;
    assert_eq!(grade["items"][0]["id"], 30071);
    assert_eq!(grade["entries_total"], 0);
    let found = api.get("/api/items?q=Swor").await;
    assert_eq!(found[0]["id"], 30071);
    assert_eq!(found[0]["in_a_grade"], true);
    assert_eq!(api.get("/api/items?q=30071").await[0]["name"], "Sword");
    assert_eq!(api.call(Method::GET, "/api/grades/2000", None).await.0, StatusCode::BAD_REQUEST);

    let helper = api.get("/api/items/30071/helper").await;
    assert_eq!(helper["old_grade"]["grade"], 5);
    assert_eq!(helper["new_grade"], 1, "grades 5, 10, 20 ... 90 are used; 1 is the lowest free");
    // a grade the draft already takes is not offered again
    let (status, _) = api
        .post(
            "/api/draft",
            json!({"by": "t", "lines": [{"target": "item_grade", "key": "30071", "action": "set", "field": "grade", "value": 1}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(api.get("/api/items/30071/helper").await["new_grade"], 2);
    db.drop().await;
}

#[tokio::test]
async fn the_draft_apply_history_and_undo_round_trip() {
    let Some(db) = Db::create().await else { return };
    let api = Api::new(&db);
    let (status, added) = api
        .post(
            "/api/draft",
            json!({"by": "tester", "lines": [{"target": "mob_drop", "key": "2133/1", "action": "set", "field": "droprate", "value": 50}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{added}");
    // the same field again replaces the line (a drag that goes on is one change)
    api.post(
        "/api/draft",
        json!({"by": "tester", "lines": [{"target": "mob_drop", "key": "2133/1", "action": "set", "field": "droprate", "value": 60}]}),
    )
    .await;
    let draft = api.get("/api/draft").await;
    assert_eq!(draft.as_array().unwrap().len(), 1);
    assert_eq!(
        (draft[0]["value"].clone(), draft[0]["now"].clone(), draft[0]["effect"].clone(), draft[0]["name"].clone()),
        (json!(60), json!(5), json!("live"), json!("Boss"))
    );

    // a refused line says why and leaves the draft as it was
    let (status, body) = api
        .post(
            "/api/draft",
            json!({"by": "tester", "lines": [{"target": "mob", "key": "2133", "action": "set", "field": "mobname", "value": "x"}]}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("cannot be edited"), "{body}");
    let (status, body) = api.post("/api/draft", json!({"by": "tester", "lines": [{"target": "mob", "key": "2133", "action": "set", "field": "hp", "value": 7000}, {"target": "mob", "key": "2133", "action": "set", "field": "exp", "value": -1}]})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(api.get("/api/draft").await.as_array().unwrap().len(), 1, "the good line of a refused request is not kept");
    assert_eq!(
        api.post("/api/draft", json!({"lines": [{"target": "mob", "key": "2133", "action": "bogus"}]})).await.0,
        StatusCode::BAD_REQUEST
    );

    // Apply needs a message
    assert_eq!(api.post("/api/apply", json!({"message": " ", "by": "tester"})).await.0, StatusCode::BAD_REQUEST);
    let (status, applied) = api.post("/api/apply", json!({"message": "tune the boss", "by": "tester"})).await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_eq!((applied["changes"].clone(), applied["effects"].clone()), (json!(1), json!(["live"])));
    assert_eq!(db.value("mob_items", "droprate", "mobid = 2133 AND itemorder = 1").await, 60);
    assert!(api.get("/api/draft").await.as_array().unwrap().is_empty());
    let edits = api.get("/api/edits").await;
    assert_eq!((edits["fields"][0]["field"].clone(), edits["fields"][0]["original"].clone()), (json!("droprate"), json!(5)));
    assert_eq!(api.get("/api/mobs/2133").await["edits"][0]["value"], 60);

    let history = api.get("/api/history").await;
    let id = history[0]["id"].as_i64().unwrap();
    assert_eq!((history[0]["message"].clone(), history[0]["made_by"].clone()), (json!("tune the boss"), json!("tester")));
    assert_eq!(api.get(&format!("/api/history/{id}")).await[0]["after"], 60);
    assert_eq!(api.call(Method::GET, "/api/history/9999", None).await.0, StatusCode::NOT_FOUND);

    let (status, undone) = api.post(&format!("/api/history/{id}/undo"), json!({"by": "tester"})).await;
    assert_eq!(status, StatusCode::OK, "{undone}");
    assert_eq!(db.value("mob_items", "droprate", "mobid = 2133 AND itemorder = 1").await, 5);
    assert_eq!(api.post(&format!("/api/history/{id}/undo"), json!({})).await.0, StatusCode::BAD_REQUEST, "already undone");

    // the draft's lines can be taken out one by one or all at once
    api.post("/api/draft", json!({"lines": [{"target": "mob", "key": "2133", "action": "set", "field": "hp", "value": 6000}]})).await;
    let seq = api.get("/api/draft").await[0]["seq"].as_i64().unwrap();
    assert_eq!(api.call(Method::DELETE, &format!("/api/draft/{seq}"), Some(json!({}))).await.0, StatusCode::OK);
    assert_eq!(api.call(Method::DELETE, &format!("/api/draft/{seq}"), Some(json!({}))).await.0, StatusCode::NOT_FOUND);
    api.post("/api/draft", json!({"lines": [{"target": "mob", "key": "2133", "action": "set", "field": "hp", "value": 6000}]})).await;
    assert_eq!(api.call(Method::DELETE, "/api/draft", Some(json!({}))).await.1["removed"], 1);
    db.drop().await;
}

#[tokio::test]
async fn spawn_areas_added_and_seen_in_the_draft() {
    let Some(db) = Db::create().await else { return };
    let api = Api::new(&db);
    let (status, body) = api
        .post(
            "/api/draft",
            json!({"by": "t", "lines": [
                {"target": "spawn_area", "key": "2/100000", "action": "add", "value": {"x1": 100, "y1": 0, "z1": 100, "x2": 120, "y2": 2, "z2": 130}},
                {"target": "spawn_mob", "key": "2/100000/1", "action": "add", "value": {"mob_id": 10, "count": 2}}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let draft = api.get("/api/draft").await;
    assert_eq!(draft.as_array().unwrap().len(), 2);
    assert_eq!(draft[0]["effect"], "restart");
    // a monster that cannot spawn is refused
    let (status, body) = api
        .post(
            "/api/draft",
            json!({"lines": [{"target": "spawn_mob", "key": "2/57/1002", "action": "add", "value": {"mob_id": 12, "count": 1}}]}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("cannot spawn"), "{body}");
    let (status, _) = api.post("/api/apply", json!({"message": "a new area", "by": "t"})).await;
    assert_eq!(status, StatusCode::OK);
    let map = api.get("/api/maps/2").await;
    assert_eq!(map["areas"].as_array().unwrap().len(), 3);
    assert_eq!(map["areas"][2]["seq"], 100000);
    db.drop().await;
}

#[tokio::test]
async fn pictures_and_names() {
    let Some(db) = Db::create().await else { return };
    let api = Api::new(&db);
    let body = |data: String| json!({"data": data, "x1": 0, "z1": 1024, "x2": 1024, "z2": 0, "source": "test.png"});
    assert_eq!(api.call(Method::GET, "/api/maps/2/image", None).await.0, StatusCode::NOT_FOUND);
    let (status, _) = api.post("/api/maps/2/image", body(STANDARD.encode(PNG))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, bytes, headers) = api.raw(Method::GET, "/api/maps/2/image", None).await;
    assert_eq!((status, bytes.as_slice(), headers["content-type"].to_str().unwrap()), (StatusCode::OK, PNG, "image/png"));
    let map = api.get("/api/maps/2").await;
    assert_eq!((map["image"]["x2"].clone(), map["image"]["source"].clone()), (json!(1024), json!("test.png")));
    assert_eq!(api.get("/api/maps").await[1]["has_image"], true);

    // refused: not a picture, not base64, no area, a map that is not there
    let (status, e) = api.post("/api/maps/2/image", body(STANDARD.encode(b"GIF89a....."))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(e["error"].as_str().unwrap().contains("PNG or a JPEG"), "{e}");
    assert_eq!(api.post("/api/maps/2/image", body("!!!".into())).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        api.post("/api/maps/2/image", json!({"data": STANDARD.encode(PNG), "x1": 5, "z1": 0, "x2": 5, "z2": 9})).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(api.post("/api/maps/77/image", body(STANDARD.encode(PNG))).await.0, StatusCode::BAD_REQUEST);

    assert_eq!(api.post("/api/maps/2/name", json!({"name": "Test Fields"})).await.0, StatusCode::OK);
    assert_eq!(api.get("/api/maps/2").await["map"]["name"], "Test Fields");
    assert_eq!(api.post("/api/maps/2/name", json!({"name": ""})).await.0, StatusCode::OK);
    assert!(api.get("/api/maps/2").await["map"]["name"].is_null());

    assert_eq!(api.call(Method::DELETE, "/api/maps/2/image", Some(json!({}))).await.0, StatusCode::OK);
    assert_eq!(api.call(Method::DELETE, "/api/maps/2/image", Some(json!({}))).await.0, StatusCode::NOT_FOUND);
    db.drop().await;
}

#[tokio::test]
async fn the_export_and_the_registry_and_the_guard() {
    let Some(db) = Db::create().await else { return };
    let api = Api::new(&db);
    let export = api.get("/api/export").await;
    assert_eq!(export["format"], "newshaiya-edits");
    let targets = api.get("/api/targets").await;
    assert!(targets.as_array().unwrap().iter().any(|t| t["name"] == "spawn_area" && t["can_add"] == true));
    assert_eq!(api.get("/api/status").await["draft_lines"], 0);
    // a change without the header is refused before it reaches the handlers
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/api/draft")
        .header("host", "localhost:30890")
        .header("content-type", "application/json")
        .body(Body::from("{\"lines\":[]}"))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 1))));
    assert_eq!(api.router.clone().oneshot(request).await.unwrap().status(), StatusCode::FORBIDDEN);
    db.drop().await;
}

#[tokio::test]
async fn a_folder_of_pictures_imports_with_names_and_reports_what_it_skips() {
    let Some(db) = Db::create().await else { return };
    let folder = std::env::temp_dir().join(format!("map-images-{:08x}", rand::random::<u32>()));
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("2.png"), PNG).unwrap();
    std::fs::write(folder.join("2.json"), r#"{"x1": 0, "z1": 0, "x2": 1024, "z2": 1024}"#).unwrap();
    std::fs::write(folder.join("1.png"), PNG).unwrap(); // no 1.json
    std::fs::write(folder.join("77.png"), PNG).unwrap(); // no map 77
    std::fs::write(folder.join("77.json"), r#"{"x1": 0, "z1": 0, "x2": 1, "z2": 1}"#).unwrap();
    std::fs::write(folder.join("names.json"), r#"{"2": "Fields", "x": "bad", "88": "nowhere"}"#).unwrap();
    let report = db.editor.import_images(&folder).await.unwrap().join("\n");
    std::fs::remove_dir_all(&folder).unwrap();
    assert!(report.contains("map 2: 2.png loaded"), "{report}");
    assert!(report.contains("map 1: 1.png skipped"), "{report}");
    assert!(report.contains("map 77: 77.png skipped: there is no map 77"), "{report}");
    assert!(
        report.contains("name \"x\" skipped") && report.contains("name of map 88 skipped") && report.contains("1 map names loaded"),
        "{report}"
    );
    let (content_type, bytes) = db.editor.map_image(2).await.unwrap().unwrap();
    assert_eq!((content_type.as_str(), bytes.as_slice()), ("image/png", PNG));
    assert!(db.editor.map_image(1).await.unwrap().is_none());
    let api = Api::new(&db);
    assert_eq!(api.get("/api/maps/2").await["map"]["name"], "Fields");
    db.drop().await;
}
