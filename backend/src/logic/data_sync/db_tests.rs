//! Source status on the dashboard and in the MCP snapshot (`--features db-tests`).

use super::test_env::*;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request};
use sqlx::PgPool;
use tower::ServiceExt;

const MCP_TOKEN: &str = "0123456789abcdef0123456789abcdef";

/// A located lawn, and a state whose environment still names a Home Assistant: the
/// returned stub, which data sync must never ask.
async fn located_with_patio_stub(pool: PgPool) -> (Stub, crate::state::AppState) {
    super::record_db_tests::located(&pool).await;
    let ha = stub(200, HA_STATE).await;
    let service = service(&config_of(&with_home_assistant(&ha.url)), pool.clone()).await;
    (ha, app_state(pool, service, true))
}

fn names(sources: &serde_json::Value) -> Vec<&str> {
    sources
        .as_array()
        .unwrap_or_else(|| panic!("not a list: {sources}"))
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect()
}

// @spec SYNC-CONN-005, SYNC-CONN-008, SYNC-MERGE-011
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_dashboard_carries_the_source_status_and_the_forecast_status(pool: PgPool) {
    let (ha, state) = located_with_patio_stub(pool).await;
    let axum::Json(dashboard) = crate::api::dashboard::get_dashboard(State(state))
        .await
        .unwrap();
    let json = serde_json::to_value(&dashboard).unwrap();
    assert!(json.get("connections").is_none(), "{json}");
    assert!(names(&json["sources"]).contains(&"Open-Meteo hourly analysis"));
    assert_eq!(
        json["forecast_status"],
        serde_json::json!({"provider": "OpenMeteo", "fetched_at": null, "last_error": null})
    );
    assert_eq!(ha.count(), 0, "data sync asks no outside source");
}

// @spec SYNC-CONN-005, SYNC-CONN-008, MCP-SNAP-009
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_mcp_snapshot_carries_the_source_status_and_the_forecast_status(pool: PgPool) {
    let (ha, state) = located_with_patio_stub(pool).await;
    let call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": "lawn_snapshot", "arguments": {}}
    });
    let request = Request::post("/mcp")
        .header(header::HOST, "turfops.lan")
        .header(header::AUTHORIZATION, format!("Bearer {MCP_TOKEN}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18")
        .body(Body::from(call.to_string()))
        .unwrap();
    let response = crate::mcp::router(state, MCP_TOKEN)
        .oneshot(request)
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let reply: serde_json::Value = serde_json::from_slice(&body)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&body)));
    let text = reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"));
    let snapshot: serde_json::Value = serde_json::from_str(text).unwrap();
    let data_sources = &snapshot["data_sources"];
    assert!(
        names(&data_sources["sources"]).contains(&"PA Avondale 2 N, PA (USCRN)"),
        "{snapshot}"
    );
    assert_eq!(data_sources["forecast"]["provider"], "OpenMeteo");
    assert!(data_sources.get("homeassistant").is_none(), "{snapshot}");
    assert_eq!(ha.count(), 0, "data sync asks no outside source");
}
