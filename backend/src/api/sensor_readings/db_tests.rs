//! Posting sensor readings through the route, against PostgreSQL.

use super::*;
use crate::api::applications::db_support::state;
use crate::api::json_errors::with_json_errors;
use crate::db::setup_queries::{self, NewSensor, RainKind, Sensor};
use crate::db::weather_queries::{fetch_state, sensors, sources};
use crate::logic::weather_sync::planning;
use crate::models::weather_record::Input;
use crate::models::{GrassType, LawnLocation};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Duration;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

async fn located(pool: &PgPool) {
    let mut profile = crate::logic::rules::test_fixtures::profile(GrassType::TallFescue);
    profile.id = None;
    profile.location = Some(LawnLocation::at(39.86, -75.79, None).unwrap());
    crate::db::queries::create_lawn_profile(pool, &profile)
        .await
        .unwrap();
}

async fn map(pool: &PgPool, name: &str, entity: Option<&str>, input: Input) -> Sensor {
    let rain_kind = (input == Input::Rain).then_some(RainKind::Counter);
    let new = NewSensor {
        name: name.into(),
        entity_id: entity.map(str::to_string),
        input,
        rain_kind,
    };
    let sensor = setup_queries::map_sensor(pool, &new).await.unwrap();
    planning::rebuild(pool).await.unwrap();
    sensor
}

async fn app(pool: &PgPool, token: Option<&str>) -> Router {
    let state = state(pool).await.posting();
    with_json_errors(mount(Router::new(), token)).with_state(state)
}

async fn post(app: &Router, bearer: Option<&str>, body: Value) -> (StatusCode, Value) {
    let mut request =
        Request::post("/api/v1/sensor-readings").header("content-type", "application/json");
    if let Some(bearer) = bearer {
        request = request.header("authorization", format!("Bearer {bearer}"));
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn answer(body: Value) -> Answer {
    serde_json::from_value(body).unwrap()
}

async fn newest(pool: &PgPool, sensor: &Sensor) -> Option<(DateTime<Utc>, f64)> {
    let source = sources::all(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.key == sensor.source_key())?;
    sensors::latest_reading(pool, source.id, sensor.input)
        .await
        .unwrap()
}

async fn state_of(pool: &PgPool, sensor: &Sensor) -> crate::models::weather_record::FetchState {
    let source = sources::all(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.key == sensor.source_key())
        .unwrap();
    fetch_state::all(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.source_id == source.id)
        .unwrap()
}

fn minutes_ago(minutes: i64) -> String {
    (Utc::now() - Duration::minutes(minutes)).to_rfc3339()
}

// @spec WX-SRC-POST-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn without_a_sensor_token_there_is_no_such_endpoint(pool: PgPool) {
    let app = app(&pool, None).await;
    let (status, body) = post(&app, Some(TOKEN), json!({"readings": []})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "There is no such API endpoint.");
}

// @spec WX-SRC-POST-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_missing_or_wrong_token_is_401_and_stores_nothing(pool: PgPool) {
    located(&pool).await;
    let probe = map(&pool, "Probe", None, Input::AirTemp).await;
    let app = app(&pool, Some(TOKEN)).await;
    let body = json!({"readings": [{"sensor": "Probe", "value": 60, "unit": "°F"}]});
    for bearer in [None, Some("0123456789abcdef0123456789abcdeX")] {
        let (status, reply) = post(&app, bearer, body.clone()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(reply["error"].is_string(), "{reply}");
    }
    assert_eq!(newest(&pool, &probe).await, None);
}

// @spec WX-SRC-POST-001, WX-SRC-POST-005, WX-SRC-POST-007, WX-SRC-POST-010, WX-SRC-POST-011
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_posted_reading_is_stored_converted_under_its_sensor(pool: PgPool) {
    located(&pool).await;
    let probe = map(&pool, "Garden probe", None, Input::SoilTemp10).await;
    let app = app(&pool, Some(TOKEN)).await;
    let observed = minutes_ago(3);
    let body = json!({"readings": [
        {"sensor": "garden PROBE ", "value": 15, "unit": "°C", "observed_at": observed},
    ]});
    let (status, reply) = post(&app, Some(TOKEN), body).await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    let reply = answer(reply);
    assert_eq!(reply.stored.len(), 1);
    assert_eq!(
        reply.stored[0].sensor, "Garden probe",
        "named as it was mapped"
    );
    let (when, value) = newest(&pool, &probe).await.unwrap();
    assert_eq!(when.to_rfc3339(), reply.stored[0].observed_at.to_rfc3339());
    assert!((value - 59.0).abs() < 1e-9);
    let state = state_of(&pool, &probe).await;
    assert!(state.last_success.is_some() && state.last_failure.is_none());

    let before = Utc::now();
    let untimed = json!({"readings": [{"sensor": "Garden probe", "value": 16, "unit": "°C"}]});
    let reply = answer(post(&app, Some(TOKEN), untimed).await.1);
    assert!(reply.stored[0].observed_at >= before - Duration::seconds(1));
}

// @spec WX-SRC-POST-004, WX-SRC-POST-010, WX-SRC-POST-011
#[sqlx::test(migrations = "src/db/migrations")]
async fn each_bad_reading_is_rejected_by_name_and_the_rest_are_stored(pool: PgPool) {
    located(&pool).await;
    let air = map(&pool, "Air", None, Input::AirTemp).await;
    map(
        &pool,
        "Patio humidity",
        Some("sensor.patio_humidity"),
        Input::Humidity,
    )
    .await;
    let app = app(&pool, Some(TOKEN)).await;
    let body = json!({"readings": [
        {"sensor": "Nobody", "value": 1, "unit": "°F"},
        {"sensor": "Patio humidity", "value": 50, "unit": "%"},
        {"sensor": "Air", "value": 290, "unit": "K", "observed_at": minutes_ago(4)},
        {"sensor": "Air", "value": 400, "unit": "°F", "observed_at": minutes_ago(3)},
        {"sensor": "Air", "value": 60, "unit": "°F", "observed_at": minutes_ago(-10)},
        {"sensor": "Air", "value": 61, "unit": "°F", "observed_at": minutes_ago(1)},
    ]});
    let (status, reply) = post(&app, Some(TOKEN), body).await;
    assert_eq!(status, StatusCode::OK);
    let reply = answer(reply);
    assert_eq!(
        reply.stored.iter().map(|t| t.index).collect::<Vec<_>>(),
        vec![5]
    );
    let rejected: Vec<_> = reply.rejected.iter().map(|r| r.index).collect();
    assert_eq!(rejected, vec![0, 1, 2, 3, 4]);
    let said = |index: usize| reply.rejected[index].problem.clone();
    assert!(said(0).contains("Nobody"), "{}", said(0));
    assert!(said(1).contains("Home Assistant"), "{}", said(1));
    assert!(said(2).contains("K"), "{}", said(2));
    assert!(said(3).contains("400"), "{}", said(3));
    assert!(said(4).contains("ahead"), "{}", said(4));
    assert_eq!(newest(&pool, &air).await.unwrap().1, 61.0);
}

// @spec WX-SRC-POST-010, WX-SRC-POST-011
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_request_whose_every_reading_is_rejected_is_400_and_says_why(pool: PgPool) {
    located(&pool).await;
    let air = map(&pool, "Air", None, Input::AirTemp).await;
    let app = app(&pool, Some(TOKEN)).await;
    let body = json!({"readings": [{"sensor": "Air", "value": 290, "unit": "K"}]});
    let (status, reply) = post(&app, Some(TOKEN), body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let reply = answer(reply);
    assert_eq!(reply.rejected.len(), 1);
    let state = state_of(&pool, &air).await;
    assert_eq!(
        state.last_error.as_deref(),
        Some(reply.rejected[0].problem.as_str())
    );
}

// @spec WX-SRC-POST-015
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_body_that_is_not_a_list_of_readings_is_400(pool: PgPool) {
    let app = app(&pool, Some(TOKEN)).await;
    let (status, reply) = post(&app, Some(TOKEN), json!({"readings": []})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(reply["error"].is_string(), "{reply}");
}

// @spec WX-SRC-POST-009
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_resent_batch_is_unchanged_not_an_error(pool: PgPool) {
    located(&pool).await;
    map(&pool, "Air", None, Input::AirTemp).await;
    let app = app(&pool, Some(TOKEN)).await;
    let body = json!({"readings": [
        {"sensor": "Air", "value": 60, "unit": "°F", "observed_at": minutes_ago(2)},
    ]});
    post(&app, Some(TOKEN), body.clone()).await;
    let (status, reply) = post(&app, Some(TOKEN), body).await;
    assert_eq!(status, StatusCode::OK);
    let reply = answer(reply);
    assert_eq!((reply.stored.len(), reply.unchanged.len()), (0, 1));
    assert!(reply.rejected.is_empty());
}

// @spec WX-SRC-POST-006, WX-SRC-POST-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_counters_batch_is_taken_oldest_first(pool: PgPool) {
    located(&pool).await;
    let gauge = map(&pool, "Gauge", None, Input::Rain).await;
    let app = app(&pool, Some(TOKEN)).await;
    let body = json!({"readings": [
        {"sensor": "Gauge", "value": 12.5, "unit": "mm", "observed_at": minutes_ago(1)},
        {"sensor": "Gauge", "value": 10.0, "unit": "mm", "observed_at": minutes_ago(20)},
        {"sensor": "Gauge", "value": 11.0, "unit": "mm", "observed_at": minutes_ago(10)},
    ]});
    let reply = answer(post(&app, Some(TOKEN), body).await.1);
    let stored: Vec<_> = reply.stored.iter().map(|t| t.index).collect();
    assert_eq!(
        stored,
        vec![0, 1, 2],
        "all accepted; the oldest is the starting point"
    );
    assert!(reply.rejected.is_empty());
    let (_, amount) = newest(&pool, &gauge).await.unwrap();
    assert!((amount - 1.5).abs() < 1e-9, "{amount}");
}

// @spec WX-SRC-POST-013
#[sqlx::test(migrations = "src/db/migrations")]
async fn readings_are_stored_before_the_lawn_has_a_location(pool: PgPool) {
    let probe = map(&pool, "Probe", None, Input::AirTemp).await;
    let app = app(&pool, Some(TOKEN)).await;
    let body = json!({"readings": [{"sensor": "Probe", "value": 55, "unit": "°F"}]});
    let (status, _) = post(&app, Some(TOKEN), body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(newest(&pool, &probe).await.unwrap().1, 55.0);
}

// @spec WX-SRC-POST-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_name_two_sensors_share_is_refused(pool: PgPool) {
    located(&pool).await;
    map(&pool, "Probe", None, Input::SoilTemp5).await;
    // Two sensors with one name can only predate the mapping check.
    sqlx::query(
        "INSERT INTO sensor (name, way_in, input) VALUES ('probe', 'Posted', 'soil_temp_10')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let app = app(&pool, Some(TOKEN)).await;
    let body = json!({"readings": [{"sensor": "Probe", "value": 50, "unit": "°F"}]});
    let reply = answer(post(&app, Some(TOKEN), body).await.1);
    assert!(
        reply.rejected[0].problem.contains("More than one sensor"),
        "{:?}",
        reply.rejected
    );
}

// @spec WX-SRC-POST-014, WX-SRC-POST-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn requests_at_once_for_one_counter_never_count_its_rain_twice(pool: PgPool) {
    located(&pool).await;
    let gauge = map(&pool, "Gauge", None, Input::Rain).await;
    let app = app(&pool, Some(TOKEN)).await;
    let start = json!({"readings": [
        {"sensor": "Gauge", "value": 0.0, "unit": "mm", "observed_at": minutes_ago(30)},
    ]});
    post(&app, Some(TOKEN), start).await;
    let mut sends = tokio::task::JoinSet::new();
    for step in 1..=10 {
        let app = app.clone();
        let body = json!({"readings": [{
            "sensor": "Gauge", "value": step as f64, "unit": "mm",
            "observed_at": minutes_ago(30 - step),
        }]});
        sends.spawn(async move { post(&app, Some(TOKEN), body).await });
    }
    sends.join_all().await;
    let total: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(value), 0) FROM weather_sensor_reading r \
         JOIN weather_source s ON s.id = r.source_id WHERE s.key = $1",
    )
    .bind(gauge.source_key())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(total <= 10.0 + 1e-9, "rain counted twice: {total}");
}

// @spec SETUP-API-013
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_sources_read_says_whether_posted_readings_are_enabled(pool: PgPool) {
    let off = state(&pool).await;
    let view = crate::api::sources::read::sources_view(&off).await.unwrap();
    assert!(!view.connections.posted_readings);
    let on = state(&pool).await.posting();
    let view = crate::api::sources::read::sources_view(&on).await.unwrap();
    assert!(view.connections.posted_readings);
}
