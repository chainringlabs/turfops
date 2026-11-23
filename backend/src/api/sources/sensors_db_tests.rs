//! Mapping the owner's sensors, against PostgreSQL and a stubbed Home Assistant.

use super::sensors::*;
use super::write_db_tests::located;
use crate::api::applications::db_support::reply;
use crate::db::setup_queries::{self, Credential, Service};
use crate::db::weather_queries::plan;
use crate::models::weather_record::Input;
use crate::state::AppState;
use crate::test_support::stub_http_sequence;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::PgPool;

type Answer = Result<Vec<SensorView>, (StatusCode, String)>;

async fn map(state: &AppState, body: serde_json::Value) -> Answer {
    let request: MappingRequest = serde_json::from_value(body).unwrap();
    match post_sensor(State(state.clone()), Json(request)).await {
        Ok(Json(sensors)) => Ok(sensors),
        Err(err) => Err(reply(err).await),
    }
}

/// One entity as Home Assistant would answer for it, reported `minutes_ago`.
pub(super) fn entity(state: &str, unit: &str, minutes_ago: i64) -> &'static str {
    let reported = (Utc::now() - Duration::minutes(minutes_ago)).to_rfc3339();
    let body = json!({
        "entity_id": "sensor.probe",
        "state": state,
        "attributes": {"unit_of_measurement": unit, "friendly_name": "Probe"},
        "last_reported": reported,
    });
    Box::leak(body.to_string().into_boxed_str())
}

/// Connects the lawn to a Home Assistant that gives these answers, in order.
pub(super) async fn home_assistant(pool: &PgPool, answers: Vec<(u16, &'static str)>) {
    let (base, _requests) = stub_http_sequence(answers).await;
    let held = Credential {
        address: Some(base),
        secret: "ha-token-0123456789".into(),
    };
    setup_queries::store_credential(pool, Service::HomeAssistant, &held)
        .await
        .unwrap();
}

async fn first_for(pool: &PgPool, input: Input) -> String {
    let placed = plan::read(pool).await.unwrap();
    let first = placed.into_iter().find(|p| p.input == input).unwrap();
    first.source.name
}

// @spec SETUP-API-009, SETUP-MAP-003, SETUP-MAP-010
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_sensor_that_posts_its_readings_is_mapped_by_name_and_input(pool: PgPool) {
    let state = located(&pool).await;
    let body = json!({"name": " Rain gauge ", "input": "rain", "rain_kind": "Counter"});
    let sensors = map(&state, body).await.unwrap();
    assert_eq!(sensors.len(), 1);
    let gauge = &sensors[0];
    assert_eq!(
        (gauge.name.as_str(), gauge.way_in),
        ("Rain gauge", "Posted")
    );
    assert_eq!((gauge.input, gauge.label), ("rain", "Rain"));
    assert_eq!(gauge.rain_kind, Some("Counter"));
    assert_eq!(
        first_for(&pool, Input::Rain).await,
        "Rain gauge",
        "the plan follows"
    );
    let listed = get_sensors(State(state.clone())).await.unwrap().0;
    assert_eq!(listed.len(), 1);

    let refused = |answer: Answer| answer.expect_err("refused");
    let (status, message) = refused(map(&state, json!({"name": "R", "input": "rain"})).await);
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        message.contains("a counter that only rises or an amount"),
        "{message}"
    );
    let daily = json!({"name": "D", "input": "daily_air"});
    assert!(refused(map(&state, daily).await)
        .1
        .contains("cannot be mapped"));
    let nameless = json!({"name": "  ", "input": "humidity"});
    assert!(refused(map(&state, nameless).await)
        .1
        .contains("Give the sensor a name"));
}

// @spec SETUP-MAP-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_second_sensor_for_an_input_needs_the_owner_to_confirm_the_replacement(pool: PgPool) {
    let state = located(&pool).await;
    map(
        &state,
        json!({"name": "Old probe", "input": "soil_temp_10"}),
    )
    .await
    .unwrap();
    let second = json!({"name": "New probe", "input": "soil_temp_10"});
    let (status, message) = map(&state, second).await.expect_err("asks first");
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        message,
        "Old probe already supplies soil temperature, 10 cm. Replace it with New probe?"
    );

    let confirmed = json!({"name": "New probe", "input": "soil_temp_10", "replace": true});
    let sensors = map(&state, confirmed).await.unwrap();
    assert_eq!(sensors.len(), 1);
    assert_eq!(sensors[0].name, "New probe");
    assert_eq!(first_for(&pool, Input::SoilTemp10).await, "New probe");
}

// @spec SETUP-MAP-004, SETUP-MAP-005, SETUP-API-010
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_home_assistant_sensor_is_checked_before_it_is_mapped(pool: PgPool) {
    let state = located(&pool).await;
    home_assistant(&pool, vec![(200, entity("16.0", "°C", 5))]).await;
    let body = json!({"name": "Patio", "entity_id": "sensor.probe", "input": "air_temp"});
    let sensors = map(&state, body).await.unwrap();
    assert_eq!(
        (sensors[0].way_in, sensors[0].entity_id.as_deref()),
        ("HomeAssistant", Some("sensor.probe"))
    );
    assert_eq!(first_for(&pool, Input::AirTemp).await, "Patio");
    assert_eq!(
        first_for(&pool, Input::DailyAir).await,
        "Patio",
        "an air sensor speaks for daily air too"
    );
}

// @spec SETUP-MAP-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_sensor_in_the_wrong_unit_or_gone_quiet_is_not_mapped(pool: PgPool) {
    let state = located(&pool).await;
    let air = json!({"name": "Patio", "entity_id": "sensor.probe", "input": "air_temp"});
    let soil = json!({"name": "Probe", "entity_id": "sensor.probe", "input": "soil_temp_10"});
    let answers = vec![
        (200, entity("289.0", "K", 5)),
        (200, entity("61.0", "°F", 90)),
        (404, r#"{"message":"Entity not found."}"#),
        (200, entity("unavailable", "°F", 5)),
        (200, entity("61.0", "°F", 90)),
    ];
    home_assistant(&pool, answers).await;
    let refused = |answer: Answer| {
        let (status, message) = answer.expect_err("refused");
        assert_eq!(status, StatusCode::BAD_REQUEST, "{message}");
        message
    };
    assert_eq!(
        refused(map(&state, air.clone()).await),
        "This sensor reports in K. For air temperature TurfOps accepts °F or °C."
    );
    assert!(refused(map(&state, air.clone()).await).contains("last reported"));
    assert!(refused(map(&state, air.clone()).await).contains("was not found in Home Assistant"));
    assert!(refused(map(&state, air).await).contains("not reporting a number"));
    assert!(setup_queries::sensors(&pool).await.unwrap().is_empty());
    // Ninety minutes is too old for air and fine for soil.
    assert_eq!(map(&state, soil).await.unwrap().len(), 1);
}

// @spec SETUP-MAP-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn one_entity_is_mapped_to_one_input(pool: PgPool) {
    let state = located(&pool).await;
    home_assistant(&pool, vec![(200, entity("61.0", "°F", 5))]).await;
    let first = json!({"name": "Probe", "entity_id": "sensor.probe", "input": "soil_temp_10"});
    map(&state, first).await.unwrap();
    let again = json!({"name": "Probe 5", "entity_id": "sensor.probe", "input": "soil_temp_5"});
    let (status, message) = map(&state, again).await.expect_err("refused");
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        message,
        "Probe is already mapped to soil temperature, 10 cm. Unmap it first."
    );
}

// @spec SETUP-API-009, SETUP-MAP-009
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_mapping_can_be_changed_and_removed_and_removal_keeps_the_sensor(pool: PgPool) {
    let state = located(&pool).await;
    let probe = map(&state, json!({"name": "Probe", "input": "soil_temp_10"}))
        .await
        .unwrap()[0]
        .id;
    map(&state, json!({"name": "Other", "input": "soil_temp_5"}))
        .await
        .unwrap();

    let change = |id: i64, body: serde_json::Value| {
        let state = state.clone();
        async move {
            let request: ChangeRequest = serde_json::from_value(body).unwrap();
            match put_sensor(State(state), Path(id), Json(request)).await {
                Ok(Json(sensors)) => Ok(sensors),
                Err(err) => Err(reply(err).await),
            }
        }
    };
    let taken = change(probe, json!({"name": "Probe", "input": "soil_temp_5"})).await;
    assert_eq!(taken.expect_err("taken").0, StatusCode::CONFLICT);
    let moved = change(
        probe,
        json!({"name": "Deep probe", "input": "soil_temp_20"}),
    )
    .await
    .unwrap();
    let deep = moved.iter().find(|s| s.id == probe).unwrap();
    assert_eq!(
        (deep.name.as_str(), deep.input),
        ("Deep probe", "soil_temp_20")
    );
    assert_eq!(first_for(&pool, Input::SoilTemp20).await, "Deep probe");
    let missing = change(9999, json!({"name": "X", "input": "humidity"})).await;
    assert_eq!(missing.expect_err("missing").0, StatusCode::NOT_FOUND);

    let left = delete_sensor(State(state.clone()), Path(probe))
        .await
        .unwrap()
        .0;
    assert_eq!(left.len(), 1);
    assert!(plan::read(&pool)
        .await
        .unwrap()
        .iter()
        .all(|p| p.input != Input::SoilTemp20));
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM sensor")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kept, 2, "the unmapped sensor's row stays");
    let again = delete_sensor(State(state), Path(probe)).await.unwrap_err();
    assert_eq!(reply(again).await.0, StatusCode::NOT_FOUND);
}
