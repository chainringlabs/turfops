//! The check before a mapping is saved, and the sensors named in the environment.

use super::sensor_check::*;
use super::sensors_db_tests::{entity, home_assistant};
use super::write_db_tests::located;
use crate::db::setup_queries::{self, Mapped};
use crate::models::weather_record::Input;
use axum::extract::State;
use axum::Json;
use sqlx::PgPool;

// @spec SETUP-API-009, SETUP-MAP-004, SETUP-MAP-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_check_shows_the_sensor_converted_and_warns_without_blocking(pool: PgPool) {
    let state = located(&pool).await;
    home_assistant(
        &pool,
        vec![(200, entity("23", "%", 5)), (200, entity("140", "%", 5))],
    )
    .await;
    let check = |input: &'static str| {
        let state = state.clone();
        async move {
            let request = CheckRequest {
                entity_id: "sensor.probe".into(),
                input: input.into(),
            };
            check_sensor(State(state), Json(request)).await.unwrap().0
        }
    };
    let moisture = check("soil_moisture_10").await;
    assert_eq!(moisture.value, Some(0.23), "percent to a fraction");
    assert_eq!(moisture.unit.as_deref(), Some("%"));
    assert!(moisture.blocking.is_empty() && moisture.warnings.is_empty());
    assert!(moisture.otherwise.is_none(), "nothing measures it here yet");
    assert!(moisture.reported_at.is_some());

    let humidity = check("humidity").await;
    assert!(humidity.blocking.is_empty(), "the owner may still save it");
    assert_eq!(humidity.warnings.len(), 1);
    assert!(humidity.warnings[0].contains("outside what humidity can be"));
    assert!(
        setup_queries::sensors(&pool).await.unwrap().is_empty(),
        "a check saves nothing"
    );
}

// @spec SETUP-MAP-011
#[sqlx::test(migrations = "src/db/migrations")]
async fn entities_named_in_the_environment_are_mapped_once(pool: PgPool) {
    let named = [
        (
            "Patio temperature",
            "sensor.patio_temperature",
            Mapped::AirTemp,
        ),
        ("Patio humidity", " ", Mapped::Humidity),
    ];
    let mapped = setup_queries::map_first_sensors(&pool, &named)
        .await
        .unwrap();
    assert_eq!(
        mapped,
        vec!["Patio temperature"],
        "an empty name is not an entity"
    );
    let sensors = setup_queries::sensors(&pool).await.unwrap();
    assert_eq!(sensors[0].input, Input::AirTemp);
    assert_eq!(
        sensors[0].entity_id.as_deref(),
        Some("sensor.patio_temperature")
    );

    // The owner unmaps it: a restart does not map it again.
    setup_queries::unmap_sensor(&pool, sensors[0].id)
        .await
        .unwrap();
    let again = setup_queries::map_first_sensors(&pool, &named)
        .await
        .unwrap();
    assert!(again.is_empty());
    assert!(setup_queries::sensors(&pool).await.unwrap().is_empty());
}
