//! A database that answers makes the server healthy, whatever the external sources do.

use super::health_check;
use crate::logic::data_sync::test_env::*;
use axum::extract::State;
use sqlx::PgPool;

// @spec SYNC-CONN-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn health_is_ok_on_the_database_alone_even_with_every_source_down(pool: PgPool) {
    let state = offline_state(pool).await;
    let axum::Json(health) = health_check(State(state)).await;
    assert_eq!(health.status, "ok");
    assert!(health.database);
}

// @spec SYNC-CONN-003, SYNC-CONN-007, SYNC-CONN-008, DEPLOY-HEALTH-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn health_reports_the_sources_in_use_and_the_forecast_status_beside_them(pool: PgPool) {
    crate::logic::data_sync::record_db_tests::located(&pool).await;
    let state = offline_state(pool).await;
    let axum::Json(health) = health_check(State(state)).await;
    let json = serde_json::to_value(&health).unwrap();
    let names: Vec<&str> = json["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"PA Avondale 2 N, PA (USCRN)"), "{json}");
    assert!(names.contains(&"Open-Meteo hourly analysis"), "{json}");
    assert_eq!(
        json["forecast"],
        serde_json::json!({"provider": "OpenMeteo", "fetched_at": null, "last_error": null}),
        "{json}"
    );
    assert!(json.get("datasources").is_none(), "{json}");
}
