//! `data-editor serve`: the page and its JSON API (`docs/plans/done/data-editing.md` sections 5 and 6). No sign-in yet: only loopback, private
//! and Tailscale addresses may connect, every request must be addressed to `localhost`, `127.0.0.1`, `[::1]` or a name in
//! `EDITOR_HOSTS` (a page that rebinds its own name to 127.0.0.1 is refused), and a request that changes anything must be a `POST` or
//! `DELETE` with a JSON content type and the header `X-Editor: 1`, which a page on another site cannot send from the operator's browser.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::targets_json;
use crate::console;
use crate::images::{MAX_IMAGE_BYTES, Rect};
use crate::store::{Editor, Line, Op, Refused};

const PAGE: &str = include_str!("page.html");
/// The longest request body: an 8 MB picture as base64 plus its fields.
const BODY_LIMIT: usize = MAX_IMAGE_BYTES / 3 * 4 + 64 * 1024;
/// The default address (`EDITOR_BIND`): loopback only, reached over `ssh -L 30890:127.0.0.1:30890` on a VPS.
pub const DEFAULT_BIND: &str = "127.0.0.1:30890";

pub struct App {
    pub editor: Editor,
    /// `EDITOR_EXPORT_FILE`: written after every Apply and Undo.
    pub export_file: Option<PathBuf>,
    /// `EDITOR_HOSTS`: host names, besides the loopback ones, a request may be addressed to (lowercase, no port).
    pub hosts: Vec<String>,
}

type Shared = Arc<App>;

/// Loopback, private (10/8, 172.16/12, 192.168/16, fc00::/7) or Tailscale (100.64.0.0/10), also as an IPv4-mapped IPv6 address.
pub fn address_allowed(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => a.is_loopback() || a.is_private() || (a.octets()[0] == 100 && (64..=127).contains(&a.octets()[1])),
        IpAddr::V6(a) => {
            a.is_loopback() || (a.segments()[0] & 0xfe00) == 0xfc00 || a.to_ipv4_mapped().is_some_and(|v4| address_allowed(IpAddr::V4(v4)))
        }
    }
}

/// Whether a request may change anything: reads are free, everything else needs the header and the JSON content type.
pub fn request_allowed(method: &Method, headers: &HeaderMap) -> bool {
    if matches!(*method, Method::GET | Method::HEAD) {
        return true;
    }
    let editor = headers.get("x-editor").is_some_and(|v| v.as_bytes() == b"1");
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim_start().to_ascii_lowercase().starts_with("application/json"));
    editor && json && matches!(*method, Method::POST | Method::DELETE)
}

/// Whether the request's `Host` is `localhost`, `127.0.0.1`, `[::1]` (any port) or one of `extra`. A request without one is refused.
pub fn host_allowed(headers: &HeaderMap, extra: &[String]) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else { return false };
    let host = host.trim().to_ascii_lowercase();
    // `[::1]:30890`, `name:30890` or a bare name; the port is whatever follows the last colon outside brackets.
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split_once(']').map(|(inside, _)| format!("[{inside}]")).unwrap_or_default(),
        None => host.split(':').next().unwrap_or_default().to_owned(),
    };
    matches!(name.as_str(), "localhost" | "127.0.0.1" | "[::1]") || extra.contains(&name)
}

/// The names of `EDITOR_HOSTS`: comma separated, lowercase, a port or brackets ignored.
pub fn parse_hosts(text: &str) -> Vec<String> {
    text.split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty())
        .map(|h| h.split(':').next().unwrap_or_default().to_owned())
        .collect()
}

async fn guard(
    State(hosts): State<Arc<Vec<String>>>,
    ConnectInfo(from): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    if !address_allowed(from.ip()) {
        return ApiError::new(
            StatusCode::FORBIDDEN,
            "this address may not use the editor (loopback, private and Tailscale addresses only)",
        )
        .into_response();
    }
    if !host_allowed(request.headers(), &hosts) {
        return ApiError::new(
            StatusCode::FORBIDDEN,
            "the request must be addressed to localhost (or a name in EDITOR_HOSTS); use an SSH tunnel",
        )
        .into_response();
    }
    if !request_allowed(request.method(), request.headers()) {
        return ApiError::new(
            StatusCode::FORBIDDEN,
            "a change must be a POST or DELETE with Content-Type: application/json and the header X-Editor: 1",
        )
        .into_response();
    }
    next.run(request).await
}

/// An error answer `{"error": "..."}`: the registry's reason for a refusal, the failure otherwise.
pub struct ApiError(StatusCode, String);

impl ApiError {
    fn new(code: StatusCode, text: impl Into<String>) -> Self {
        Self(code, text.into())
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        if let Some(Refused(m)) = e.downcast_ref::<Refused>() {
            return Self(StatusCode::BAD_REQUEST, m.clone());
        }
        tracing::error!("{e:#}");
        Self(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}

type Reply = Result<Json<Value>, ApiError>;

fn not_found(what: impl std::fmt::Display) -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, format!("there is no {what}"))
}

pub fn router(app: Shared) -> Router {
    let hosts = Arc::new(app.hosts.clone());
    Router::new()
        .route("/", get(|| async { Html(PAGE) }))
        .route("/favicon.ico", get(|| async { StatusCode::NO_CONTENT }))
        .route("/api/targets", get(|| async { Json(targets_json()) }))
        .route("/api/status", get(status))
        .route("/api/maps", get(maps))
        .route("/api/maps/{id}", get(map))
        .route("/api/maps/{id}/blocked", get(blocked))
        .route("/api/maps/{id}/image", get(image).post(set_image).delete(delete_image))
        .route("/api/maps/{id}/name", post(set_name))
        .route("/api/mobs/{id}", get(mob))
        .route("/api/grades/{grade}", get(grade))
        .route("/api/items", get(items))
        .route("/api/items/{id}/helper", get(helper))
        .route("/api/edits", get(edits))
        .route("/api/draft", get(draft).post(draft_post).delete(draft_clear))
        .route("/api/draft/{seq}", delete(draft_remove))
        .route("/api/apply", post(apply))
        .route("/api/history", get(history))
        .route("/api/history/{id}", get(history_detail))
        .route("/api/history/{id}/undo", post(undo))
        .route("/api/servers", get(servers))
        .route("/api/servers/{id}/reload", post(reload))
        .route("/api/servers/{id}/restart", post(restart))
        .route("/api/servers/{id}/cancel", post(cancel))
        .route("/api/export", get(export))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(hosts, guard))
        .with_state(app)
}

/// Serves the page until Ctrl+C or SIGTERM.
pub async fn serve(editor: Editor, bind: &str, export_file: Option<PathBuf>, hosts: Vec<String>) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(bind).await.with_context(|| format!("binding {bind} (EDITOR_BIND)"))?;
    tracing::info!("data editor on http://{}/ (loopback, private and Tailscale addresses, addressed to localhost)", listener.local_addr()?);
    let app = router(Arc::new(App { editor, export_file, hosts }));
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async {
            let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
        })
        .await?;
    Ok(())
}

async fn status(State(app): State<Shared>) -> Reply {
    Ok(Json(serde_json::to_value(app.editor.status().await?).map_err(anyhow::Error::from)?))
}

async fn maps(State(app): State<Shared>) -> Reply {
    Ok(Json(app.editor.api_maps().await?))
}

async fn map(State(app): State<Shared>, Path(id): Path<i64>) -> Reply {
    Ok(Json(app.editor.api_map(id).await?.ok_or_else(|| not_found(format!("map {id}")))?))
}

async fn blocked(State(app): State<Shared>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    let (size, bits) = app.editor.api_blocked(id).await?.ok_or_else(|| not_found(format!("map {id}")))?;
    Ok((
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream")),
            (header::HeaderName::from_static("x-map-size"), HeaderValue::from(size)),
        ],
        bits,
    )
        .into_response())
}

async fn image(State(app): State<Shared>, Path(id): Path<i64>) -> Result<Response, ApiError> {
    let (content_type, bytes) = app.editor.map_image(id).await?.ok_or_else(|| not_found(format!("picture for map {id}")))?;
    let content_type = HeaderValue::from_str(&content_type).map_err(anyhow::Error::from)?;
    Ok(([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, HeaderValue::from_static("no-cache"))], bytes).into_response())
}

#[derive(Deserialize)]
struct ImageBody {
    /// The file, as base64.
    data: String,
    #[serde(flatten)]
    rect: Rect,
    source: Option<String>,
}

async fn set_image(State(app): State<Shared>, Path(id): Path<i64>, Json(body): Json<ImageBody>) -> Reply {
    let bytes = STANDARD.decode(body.data.trim()).map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "the picture is not valid base64"))?;
    app.editor.set_map_image(id, &bytes, body.rect, body.source.as_deref().unwrap_or("upload")).await?;
    Ok(Json(json!({"ok": true})))
}

async fn delete_image(State(app): State<Shared>, Path(id): Path<i64>) -> Reply {
    if !app.editor.delete_map_image(id).await? {
        return Err(not_found(format!("picture for map {id}")));
    }
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct NameBody {
    name: String,
}

async fn set_name(State(app): State<Shared>, Path(id): Path<i64>, Json(body): Json<NameBody>) -> Reply {
    app.editor.set_map_name(id, &body.name).await?;
    Ok(Json(json!({"ok": true})))
}

async fn mob(State(app): State<Shared>, Path(id): Path<i64>) -> Reply {
    Ok(Json(app.editor.api_mob(id).await?.ok_or_else(|| not_found(format!("monster {id}")))?))
}

async fn grade(State(app): State<Shared>, Path(grade): Path<i64>) -> Reply {
    if !(0..=1024).contains(&grade) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "a grade is 0 to 1024"));
    }
    Ok(Json(app.editor.api_grade(grade).await?))
}

#[derive(Deserialize)]
struct ItemQuery {
    #[serde(default)]
    q: String,
}

async fn items(State(app): State<Shared>, Query(query): Query<ItemQuery>) -> Reply {
    Ok(Json(app.editor.api_items(&query.q).await?))
}

async fn helper(State(app): State<Shared>, Path(id): Path<i64>) -> Reply {
    Ok(Json(app.editor.api_helper(id).await?))
}

async fn edits(State(app): State<Shared>) -> Reply {
    Ok(Json(app.editor.api_edits().await?))
}

async fn draft(State(app): State<Shared>) -> Reply {
    Ok(Json(app.editor.api_draft().await?))
}

#[derive(Deserialize)]
struct DraftBody {
    #[serde(default)]
    by: String,
    lines: Vec<DraftIn>,
}

#[derive(Deserialize)]
struct DraftIn {
    target: String,
    key: String,
    /// `set`, `unset`, `add` or `remove`.
    action: String,
    field: Option<String>,
    value: Option<Value>,
}

impl DraftIn {
    fn line(self) -> Result<Line, ApiError> {
        let bad = |m: &str| ApiError::new(StatusCode::BAD_REQUEST, format!("{} {}: {m}", self.target, self.key));
        let op = match self.action.as_str() {
            "set" => Op::Set {
                field: self.field.clone().ok_or_else(|| bad("a set needs a field"))?,
                value: self.value.clone().ok_or_else(|| bad("a set needs a value"))?,
            },
            "unset" => Op::Unset { field: self.field.clone().ok_or_else(|| bad("an unset needs a field"))? },
            "add" => Op::Add { fields: self.value.clone().ok_or_else(|| bad("an add needs the row's fields"))? },
            "remove" => Op::Remove,
            other => return Err(bad(&format!("unknown action {other:?}"))),
        };
        Ok(Line { target: self.target, key: self.key, op })
    }
}

/// The name typed on the page, or `web` when it was left empty.
fn who(by: &str) -> String {
    let by = by.trim();
    if by.is_empty() { "web".to_owned() } else { by.chars().take(40).collect() }
}

async fn draft_post(State(app): State<Shared>, Json(body): Json<DraftBody>) -> Reply {
    let lines = body.lines.into_iter().map(DraftIn::line).collect::<Result<Vec<_>, _>>()?;
    let added = app.editor.draft_put(&lines, &who(&body.by)).await?;
    Ok(Json(json!({"added": added})))
}

async fn draft_remove(State(app): State<Shared>, Path(seq): Path<i32>) -> Reply {
    if !app.editor.draft_remove(seq).await? {
        return Err(not_found(format!("draft line {seq}")));
    }
    Ok(Json(json!({"ok": true})))
}

async fn draft_clear(State(app): State<Shared>) -> Reply {
    Ok(Json(json!({"removed": app.editor.draft_clear().await?})))
}

#[derive(Deserialize)]
struct ApplyBody {
    message: String,
    #[serde(default)]
    by: String,
}

/// The answer of an Apply or Undo: the change set and its effects, and whether the automatic export worked.
async fn after_change(app: &App, applied: crate::store::Applied) -> Reply {
    let mut answer = serde_json::to_value(&applied).map_err(anyhow::Error::from)?;
    if let Some(path) = &app.export_file
        && let Err(e) = app.editor.export_to(path, false).await
    {
        answer["export_error"] = json!(format!("the change is applied, but the export to {} failed: {e:#}", path.display()));
    }
    Ok(Json(answer))
}

async fn apply(State(app): State<Shared>, Json(body): Json<ApplyBody>) -> Reply {
    let applied = app.editor.apply_draft(&body.message, &who(&body.by)).await?;
    after_change(&app, applied).await
}

#[derive(Deserialize, Default)]
struct ByBody {
    #[serde(default)]
    by: String,
}

async fn undo(State(app): State<Shared>, Path(id): Path<i32>, Json(body): Json<ByBody>) -> Reply {
    let applied = app.editor.undo(id, &who(&body.by)).await?;
    after_change(&app, applied).await
}

async fn history(State(app): State<Shared>) -> Reply {
    Ok(Json(serde_json::to_value(app.editor.history(200).await?).map_err(anyhow::Error::from)?))
}

async fn history_detail(State(app): State<Shared>, Path(id): Path<i32>) -> Reply {
    let lines = app.editor.history_detail(id).await?;
    if lines.is_empty() {
        return Err(not_found(format!("change set #{id}")));
    }
    Ok(Json(serde_json::to_value(lines).map_err(anyhow::Error::from)?))
}

async fn servers(State(app): State<Shared>) -> Reply {
    Ok(Json(app.editor.api_servers().await?))
}

/// Sends a command to a game server through the database and returns its answer.
async fn console_line(app: &App, id: i64, line: &str) -> Reply {
    let server = u8::try_from(id).map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "a game server id is 0 to 255"))?;
    let answer =
        console::request(&app.editor.pool, server, line).await.map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, format!("{e:#}")))?;
    Ok(Json(json!({"answer": answer})))
}

async fn reload(State(app): State<Shared>, Path(id): Path<i64>) -> Reply {
    console_line(&app, id, "reload").await
}

#[derive(Deserialize)]
struct RestartBody {
    minutes: u32,
}

async fn restart(State(app): State<Shared>, Path(id): Path<i64>, Json(body): Json<RestartBody>) -> Reply {
    if !(1..=1440).contains(&body.minutes) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "minutes are 1 to 1440"));
    }
    console_line(&app, id, &format!("restart {}", body.minutes)).await
}

async fn cancel(State(app): State<Shared>, Path(id): Path<i64>) -> Reply {
    console_line(&app, id, "quit cancel").await
}

async fn export(State(app): State<Shared>) -> Result<Response, ApiError> {
    let doc = app.editor.export(false).await?;
    let text = serde_json::to_string_pretty(&doc).map_err(anyhow::Error::from)?;
    Ok(([(header::CONTENT_TYPE, "application/json"), (header::CONTENT_DISPOSITION, "attachment; filename=\"edits.json\"")], text)
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use tower::ServiceExt;

    #[test]
    fn who_may_connect() {
        for ok in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.0.10",
            "100.64.0.1",
            "100.127.255.255",
            "::1",
            "fd00::1",
            "fc00::1",
            "::ffff:192.168.1.5",
            "::ffff:100.100.1.1",
        ] {
            assert!(address_allowed(ok.parse().unwrap()), "{ok}");
        }
        for no in [
            "8.8.8.8",
            "172.32.0.1",
            "172.15.0.1",
            "100.63.255.255",
            "100.128.0.1",
            "169.254.1.1",
            "2001:db8::1",
            "fe80::1",
            "::ffff:8.8.8.8",
        ] {
            assert!(!address_allowed(no.parse().unwrap()), "{no}");
        }
    }

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, HeaderValue::from_static(v));
        }
        h
    }

    #[test]
    fn a_change_needs_the_header_and_json() {
        let ok = headers(&[("x-editor", "1"), ("content-type", "application/json")]);
        assert!(request_allowed(&Method::POST, &ok));
        assert!(request_allowed(&Method::DELETE, &ok));
        assert!(request_allowed(&Method::POST, &headers(&[("x-editor", "1"), ("content-type", "Application/JSON; charset=utf-8")])));
        assert!(request_allowed(&Method::GET, &HeaderMap::new()), "reads are free");
        assert!(!request_allowed(&Method::POST, &headers(&[("content-type", "application/json")])), "no header");
        assert!(!request_allowed(&Method::POST, &headers(&[("x-editor", "1")])), "no content type");
        assert!(!request_allowed(&Method::POST, &headers(&[("x-editor", "1"), ("content-type", "text/plain")])), "a form or text post");
        assert!(!request_allowed(&Method::POST, &headers(&[("x-editor", "yes"), ("content-type", "application/json")])));
        assert!(!request_allowed(&Method::PUT, &ok), "only POST and DELETE change things");
    }

    /// The guard in front of a router that answers 200 to everything; no database.
    fn guarded() -> Router {
        Router::new()
            .route("/api/x", get(|| async { "read" }).post(|| async { "changed" }))
            .layer(middleware::from_fn_with_state(Arc::new(vec!["editor.example".to_owned()]), guard))
    }

    async fn call(from: &str, method: Method, headers: &[(&str, &str)]) -> StatusCode {
        let mut request = Request::builder().method(method).uri("/api/x").header("host", "localhost:30890");
        for (k, v) in headers {
            request = request.header(*k, *v);
        }
        let mut request = request.body(Body::empty()).unwrap();
        request.extensions_mut().insert(ConnectInfo(SocketAddr::new(from.parse().unwrap(), 50000)));
        guarded().oneshot(request).await.unwrap().status()
    }

    #[tokio::test]
    async fn the_guard_refuses_outsiders_and_bare_posts() {
        assert_eq!(call("192.168.1.9", Method::GET, &[]).await, StatusCode::OK);
        assert_eq!(call("8.8.8.8", Method::GET, &[]).await, StatusCode::FORBIDDEN);
        assert_eq!(call("8.8.8.8", Method::POST, &[("x-editor", "1"), ("content-type", "application/json")]).await, StatusCode::FORBIDDEN);
        assert_eq!(call("127.0.0.1", Method::POST, &[("content-type", "application/json")]).await, StatusCode::FORBIDDEN);
        assert_eq!(call("127.0.0.1", Method::POST, &[("x-editor", "1"), ("content-type", "application/json")]).await, StatusCode::OK);
    }

    #[test]
    fn only_localhost_and_listed_names_are_hosts() {
        let listed = vec!["editor.example".to_owned()];
        for ok in [
            "localhost",
            "localhost:30890",
            "LocalHost:1",
            "127.0.0.1:30890",
            "127.0.0.1",
            "[::1]:30890",
            "[::1]",
            "editor.example:30890",
            "Editor.Example",
        ] {
            assert!(host_allowed(&headers_from(ok), &listed), "{ok}");
        }
        for no in [
            "evil.example",
            "evil.example:30890",
            "localhost.evil.example",
            "192.168.1.5:30890",
            "[::2]:30890",
            "127.0.0.2",
            "",
            "localhost@evil.example",
            "editor.example.evil",
        ] {
            assert!(!host_allowed(&headers_from(no), &listed), "{no}");
        }
        assert!(!host_allowed(&HeaderMap::new(), &listed), "no Host header");
        assert_eq!(parse_hosts(" Editor.Example:80 , ,b.example"), ["editor.example", "b.example"]);
    }

    fn headers_from(host: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, HeaderValue::from_str(host).unwrap());
        h
    }

    #[tokio::test]
    async fn a_request_addressed_to_another_name_is_refused_even_from_loopback() {
        // A page that rebinds its own name to 127.0.0.1 arrives from loopback, with its own name as the Host.
        let mut request =
            Request::builder().method(Method::GET).uri("/api/x").header("host", "rebound.evil.example:30890").body(Body::empty()).unwrap();
        request.extensions_mut().insert(ConnectInfo(SocketAddr::new("127.0.0.1".parse().unwrap(), 50000)));
        assert_eq!(guarded().oneshot(request).await.unwrap().status(), StatusCode::FORBIDDEN);
        let mut request =
            Request::builder().method(Method::GET).uri("/api/x").header("host", "editor.example").body(Body::empty()).unwrap();
        request.extensions_mut().insert(ConnectInfo(SocketAddr::new("127.0.0.1".parse().unwrap(), 50000)));
        assert_eq!(guarded().oneshot(request).await.unwrap().status(), StatusCode::OK, "a listed name passes");
    }
}
