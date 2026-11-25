//! The health response's shape, and a database that does not answer.

use super::health_check;
use crate::logic::data_sync::test_env::*;
use axum::extract::State;

// @spec SYNC-CONN-003, SYNC-CONN-008, DEPLOY-HEALTH-001
#[tokio::test]
async fn health_reports_status_version_database_sources_forecast_and_mcp() {
    for (mcp_enabled, mcp) in [(true, "enabled"), (false, "disabled")] {
        let service = service(&config(&[]), unreachable_pool()).await;
        let state = app_state(unreachable_pool(), service, mcp_enabled);
        let axum::Json(health) = health_check(State(state)).await;
        let json = serde_json::to_value(&health).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            ["database", "forecast", "mcp", "sources", "status", "version"]
        );
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(json["mcp"], mcp);
        assert_eq!(
            json["sources"],
            serde_json::json!([]),
            "no database, no sources"
        );
        assert!(
            json["forecast"].is_null(),
            "no database, no forecast status: {json}"
        );
    }
}

// @spec SYNC-CONN-004
#[tokio::test]
async fn health_is_degraded_when_the_database_does_not_answer() {
    let state = offline_state(unreachable_pool()).await;
    let started = std::time::Instant::now();
    let axum::Json(health) = health_check(State(state)).await;
    assert_eq!(health.status, "degraded");
    assert!(!health.database);
    assert!(started.elapsed() < std::time::Duration::from_secs(4));
}
