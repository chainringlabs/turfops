//! Shared helpers for the MCP tests: an `AppState` over a given pool, JSON-RPC over the
//! real `/mcp` router, and log capture.

use crate::clock::Clock;
use crate::config::Config;
use crate::logic::data_sync::DataSyncService;
use crate::state::AppState;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

pub const TOKEN: &str = "0123456789abcdef0123456789abcdef";
pub const PROTOCOL: &str = "2025-06-18";

/// A configuration with no Home Assistant and no forecast.
pub fn config() -> Config {
    serde_json::from_value(json!({
        "lawn": {"name": "Test lawn", "grass_type": "TallFescue", "usda_zone": "7a",
                 "soil_type": null, "lawn_size_sqft": null, "irrigation_type": null},
        "homeassistant": {"url": "http://127.0.0.1:1", "token": "",
                          "temperature_entity": "sensor.t", "humidity_entity": "sensor.h"},
        "openweathermap": null,
        "openrouter": null,
        "server": {"host": "127.0.0.1", "port": 3000, "cors_allowed_origin": null},
        "database": {"host": "127.0.0.1", "port": 1, "name": "none", "user": "none",
                     "password": "none", "max_connections": 1},
        "mcp": {"token": TOKEN}
    }))
    .expect("test config")
}

/// A pool that never connects: every query fails quickly with a database error.
pub fn unreachable_pool() -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(300))
        .connect_lazy("postgres://nobody@127.0.0.1:1/none")
        .expect("lazy pool")
}

/// App state over `pool`, answering with a fixed reading for current conditions.
pub async fn state(pool: PgPool) -> AppState {
    let sync = DataSyncService::initialize(&config(), pool.clone())
        .await
        .fixed_reading();
    AppState::new(pool, sync, None, true, Clock::System)
}

/// A POST to `/mcp` carrying the bearer token and the headers an MCP client sends.
pub fn post(body: &Value) -> axum::http::request::Builder {
    post_to("turfops.lan:3000", body)
}

/// `post` addressed to `host`.
pub fn post_to(host: &str, body: &Value) -> axum::http::request::Builder {
    Request::builder()
        .method("POST")
        .uri("/mcp")
        .header(header::HOST, host)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", PROTOCOL)
        .header(header::CONTENT_LENGTH, body.to_string().len())
}

/// Send `request` to `app`; the status, the headers and the body as text.
pub async fn send(
    app: Router,
    request: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = app.oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    (
        parts.status,
        parts.headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

/// One JSON-RPC request to the real `/mcp` router; the JSON-RPC response.
pub async fn rpc(state: &AppState, method: &str, params: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let request = post(&body).body(Body::from(body.to_string())).unwrap();
    let (status, _, text) = send(super::router(state.clone(), TOKEN), request).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

/// The outcome of one tool call.
#[derive(Debug)]
pub struct ToolReply {
    pub is_error: bool,
    pub text: String,
}

impl ToolReply {
    /// The reply's JSON body.
    #[cfg(feature = "db-tests")]
    pub fn json(&self) -> Value {
        assert!(!self.is_error, "tool error: {}", self.text);
        serde_json::from_str(&self.text).unwrap_or_else(|e| panic!("{e}: {}", self.text))
    }
}

/// Call `name` with `arguments` over JSON-RPC. Panics on a protocol-level error.
pub async fn call_tool(state: &AppState, name: &str, arguments: Value) -> ToolReply {
    let response = rpc(
        state,
        "tools/call",
        json!({"name": name, "arguments": arguments}),
    )
    .await;
    assert!(
        response.get("error").is_none(),
        "{name}: JSON-RPC error {response}"
    );
    let result = &response["result"];
    let content = result["content"].as_array().expect("content");
    assert_eq!(content.len(), 1, "{name}: one content block");
    assert_eq!(content[0]["type"], "text");
    ToolReply {
        is_error: result["isError"].as_bool().unwrap_or(false),
        text: content[0]["text"].as_str().unwrap().to_string(),
    }
}

/// Everything logged at any level while `f` runs, as text.
pub fn logs_of<R>(f: impl FnOnce() -> R) -> (R, String) {
    let logs = Logs::capture();
    let result = f();
    (result, logs.text())
}

/// Log capture on the current thread (a `#[tokio::test]` runs its tasks there) until
/// dropped.
pub struct Logs {
    buffer: Arc<Mutex<Vec<u8>>>,
    _guard: tracing::subscriber::DefaultGuard,
}

impl Logs {
    pub fn capture() -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || Capture(writer.clone()))
            .finish();
        crate::test_support::keep_tracing_interest_shared();
        Self {
            buffer,
            _guard: tracing::subscriber::set_default(subscriber),
        }
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.buffer.lock().unwrap()).into_owned()
    }
}

struct Capture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
