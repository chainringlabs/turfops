//! A database that answers makes the server healthy, whatever the external sources do.

use super::health_check;
use crate::logic::data_sync::test_env::*;
use axum::extract::State;
use sqlx::PgPool;

// @spec SYNC-CONN-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn health_is_ok_on_the_database_alone_even_with_every_source_down(pool: PgPool) {
    let ha = stub(500, "{}").await;
    let service = service(&config_of(&with_home_assistant(&ha.url)), pool.clone()).await;
    let state = app_state(pool, service, false);

    let axum::Json(health) = health_check(State(state)).await;
    assert_eq!(health.status, "ok");
    assert!(health.database);
    let sources = &health.datasources;
    assert!(!sources.soildata && !sources.homeassistant);
}

// @spec SYNC-CONN-008, SYNC-CONN-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn health_reports_the_forecast_status_beside_the_sources(pool: PgPool) {
    let state = offline_state(pool).await;
    let axum::Json(health) = health_check(State(state)).await;
    let json = serde_json::to_value(&health).unwrap();
    assert_eq!(
        json["forecast"],
        serde_json::json!({"provider": "OpenMeteo", "fetched_at": null, "last_error": null}),
        "{json}"
    );
    assert!(
        json["datasources"].get("openweathermap").is_none(),
        "{json}"
    );
}
