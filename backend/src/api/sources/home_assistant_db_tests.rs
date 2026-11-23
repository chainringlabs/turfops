//! The Home Assistant connection endpoints against PostgreSQL and a local stub.

use super::home_assistant::*;
use crate::api::applications::db_support::{reply, state};
use crate::db::setup_queries::{self, NewSensor, Service};
use crate::models::weather_record::Input;
use crate::state::AppState;
use crate::test_support::stub_http_sequence;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use sqlx::PgPool;

const TOKEN: &str = "ha-long-lived-token-0123456789";
const CONFIG: &str = r#"{"location_name":"Home","version":"2026.11.2"}"#;
const STATES: &str = r#"[
{"entity_id":"sensor.zebra_power","state":"120","attributes":{"friendly_name":"Zebra power",
 "unit_of_measurement":"W","device_class":"power"},"last_updated":"2026-11-23T14:00:00+00:00"},
{"entity_id":"sensor.rain_total","state":"812.4","attributes":{"friendly_name":"Rain total",
 "unit_of_measurement":"mm","device_class":"precipitation","state_class":"total_increasing"},
 "last_updated":"2026-11-23T14:00:00+00:00"},
{"entity_id":"sensor.rain_event","state":"1.2","attributes":{"friendly_name":"Rain event",
 "unit_of_measurement":"mm","device_class":"precipitation","state_class":"measurement"},
 "last_updated":"2026-11-23T14:00:00+00:00"},
{"entity_id":"sensor.patio_humidity","state":"55","attributes":{"friendly_name":"Patio humidity",
 "unit_of_measurement":"%","device_class":"humidity"},"last_updated":"2026-11-23T14:00:00+00:00"},
{"entity_id":"sensor.bed_moisture","state":"23","attributes":{"friendly_name":"Bed moisture",
 "unit_of_measurement":"%","device_class":"moisture"},"last_updated":"2026-11-23T14:00:00+00:00"},
{"entity_id":"sensor.patio_temperature","state":"61.5","attributes":{"friendly_name":"Patio temperature",
 "unit_of_measurement":"°F","device_class":"temperature"},"last_updated":"2026-11-23T14:00:00+00:00"},
{"entity_id":"sensor.attic_battery","state":"87","attributes":{"friendly_name":"Attic battery",
 "unit_of_measurement":"%","device_class":"battery"},"last_updated":"2026-11-23T14:00:00+00:00"}
]"#;

async fn connect(state: &AppState, address: &str) -> Result<ConnectionView, (StatusCode, String)> {
    let request = ConnectionRequest {
        address: address.into(),
        token: TOKEN.into(),
    };
    match put_connection(State(state.clone()), Json(request)).await {
        Ok(Json(view)) => Ok(view),
        Err(err) => Err(reply(err).await),
    }
}

// @spec SETUP-API-007, SETUP-CRED-001, SETUP-CRED-003, SETUP-HA-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_connection_is_stored_shown_without_its_token_and_forgotten(pool: PgPool) {
    let state = state(&pool).await;
    let none = get_connection(State(state.clone())).await.unwrap().0;
    assert!(!none.connected && none.address.is_none());

    let view = connect(&state, " http://ha.example:8123/ ").await.unwrap();
    assert!(view.connected);
    assert_eq!(view.address.as_deref(), Some("http://ha.example:8123"));
    assert!(!serde_json::to_string(&view).unwrap().contains(TOKEN));
    let stored = setup_queries::credential(&pool, Service::HomeAssistant)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.secret, TOKEN);

    let gone = delete_connection(State(state)).await.unwrap().0;
    assert!(!gone.connected && gone.address.is_none());
    assert!(setup_queries::credential(&pool, Service::HomeAssistant)
        .await
        .unwrap()
        .is_none());
}

// @spec SETUP-HA-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn an_address_that_is_not_http_or_https_is_refused_and_nothing_is_stored(pool: PgPool) {
    let state = state(&pool).await;
    let (status, message) = connect(&state, "homeassistant.local:8123")
        .await
        .err()
        .unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        message.contains("starts with http:// or https://"),
        "{message}"
    );
    assert!(
        !setup_queries::credential_ever_stored(&pool, Service::HomeAssistant)
            .await
            .unwrap()
    );
}

async fn test(
    state: &AppState,
    body: serde_json::Value,
) -> Result<TestResult, (StatusCode, String)> {
    let request: TestRequest = serde_json::from_value(body).unwrap();
    match test_connection(State(state.clone()), Json(request)).await {
        Ok(Json(result)) => Ok(result),
        Err(err) => Err(reply(err).await),
    }
}

// @spec SETUP-API-007, SETUP-HA-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_test_says_what_answered_and_how_many_sensors_it_has_and_saves_nothing(pool: PgPool) {
    let state = state(&pool).await;
    let (base, _requests) = stub_http_sequence(vec![(200, CONFIG), (200, STATES)]).await;
    let body = serde_json::json!({"address": base, "token": TOKEN});
    let result = test(&state, body).await.unwrap();
    assert_eq!(result.answered.location_name, "Home");
    assert_eq!(result.answered.version, "2026.11.2");
    assert_eq!(result.numeric_sensors, 7);
    assert!(
        !setup_queries::credential_ever_stored(&pool, Service::HomeAssistant)
            .await
            .unwrap()
    );
}

// @spec SETUP-FAIL-002, SETUP-API-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_failed_test_says_whether_nothing_answered_or_the_token_was_refused(pool: PgPool) {
    let state = state(&pool).await;
    let (status, message) = test(&state, serde_json::json!({})).await.err().unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("isn't connected yet"), "{message}");

    let nowhere = serde_json::json!({"address": "http://127.0.0.1:1", "token": TOKEN});
    let (_, message) = test(&state, nowhere).await.err().unwrap();
    assert_eq!(message, "Home Assistant did not answer at that address.");

    // The stored connection is the one tried when none is given.
    let (base, _requests) = stub_http_sequence(vec![(401, "401: Unauthorized")]).await;
    connect(&state, &base).await.unwrap();
    let (_, message) = test(&state, serde_json::json!({})).await.err().unwrap();
    assert_eq!(message, "Home Assistant refused the token.");
    assert!(!message.contains(TOKEN));
}

// @spec SETUP-API-008, SETUP-MAP-001, SETUP-MAP-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn entities_that_look_like_weather_come_first_each_with_what_it_seems_to_measure(
    pool: PgPool,
) {
    let state = state(&pool).await;
    let (base, _requests) = stub_http_sequence(vec![(200, STATES)]).await;
    connect(&state, &base).await.unwrap();
    let mapped = NewSensor {
        name: "Patio".into(),
        entity_id: Some("sensor.patio_humidity".into()),
        input: Input::Humidity,
        rain_kind: None,
    };
    setup_queries::map_sensor(&pool, &mapped).await.unwrap();

    let entities = get_entities(State(state)).await.unwrap().0;
    let listed: Vec<_> = entities
        .iter()
        .map(|e| (e.friendly_name.as_str(), e.suggestion))
        .collect();
    assert_eq!(
        listed,
        vec![
            ("Bed moisture", Some(Suggestion::SoilMoisture)),
            ("Patio humidity", Some(Suggestion::Humidity)),
            ("Patio temperature", None),
            ("Rain event", Some(Suggestion::RainAmount)),
            ("Rain total", Some(Suggestion::RainCounter)),
            ("Attic battery", None),
            ("Zebra power", None),
        ]
    );
    assert_eq!(entities[1].mapped_to, Some("humidity"));
    assert_eq!(entities[0].mapped_to, None);
    assert_eq!(
        (entities[2].value, entities[2].unit.as_deref()),
        (61.5, Some("°F"))
    );
}

// @spec SETUP-API-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_no_connection_the_entity_list_says_to_connect(pool: PgPool) {
    let err = get_entities(State(state(&pool).await)).await.unwrap_err();
    let (status, message) = reply(err).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("isn't connected yet"));
}

// @spec SETUP-CRED-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn connections_in_the_environment_are_imported_once_and_never_again(pool: PgPool) {
    let ha = Some(("http://ha.example:8123", TOKEN));
    let imported = setup_queries::import_once(&pool, ha, Some("owm-key"))
        .await
        .unwrap();
    assert_eq!(
        imported,
        vec![Service::HomeAssistant, Service::OpenWeatherMap]
    );
    let held = setup_queries::credential(&pool, Service::HomeAssistant)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(held.address.as_deref(), Some("http://ha.example:8123"));

    // The owner forgets one: a restart with the same environment does not bring it back.
    setup_queries::forget_credential(&pool, Service::HomeAssistant)
        .await
        .unwrap();
    let again = setup_queries::import_once(&pool, ha, Some("another-key"))
        .await
        .unwrap();
    assert!(again.is_empty());
    assert!(setup_queries::credential(&pool, Service::HomeAssistant)
        .await
        .unwrap()
        .is_none());
    let owm = setup_queries::credential(&pool, Service::OpenWeatherMap)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(owm.secret, "owm-key", "the stored key stands");

    // Nothing in the environment imports nothing.
    let fresh = setup_queries::import_once(&pool, Some(("", "")), None)
        .await
        .unwrap();
    assert!(fresh.is_empty());
}
