//! Reading the owner's Home Assistant sensors against PostgreSQL and a stubbed Home
//! Assistant.

use super::planning;
use super::schedule_db_tests::{at, located, state_of};
use super::sensor_read::{Read, SensorReader};
use crate::datasources::homeassistant::connection::Unusable;
use crate::db::setup_queries::{self, Credential, NewSensor, RainKind, Sensor, Service};
use crate::db::weather_queries::sensors;
use crate::models::weather_record::Input;
use crate::test_support::stub_http_sequence;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use tokio::sync::mpsc::UnboundedReceiver;

/// One entity as Home Assistant answers for it.
pub(super) fn answer(
    entity_id: &str,
    state: &str,
    unit: &str,
    reported: DateTime<Utc>,
) -> &'static str {
    let body = json!({
        "entity_id": entity_id,
        "state": state,
        "attributes": {"unit_of_measurement": unit},
        "last_updated": reported.to_rfc3339(),
        "last_reported": reported.to_rfc3339(),
    });
    Box::leak(body.to_string().into_boxed_str())
}

/// Connects the lawn to a Home Assistant that gives these answers, in order.
pub(super) async fn connect(
    pool: &PgPool,
    answers: Vec<(u16, &'static str)>,
) -> UnboundedReceiver<String> {
    let (base, requests) = stub_http_sequence(answers).await;
    let held = Credential {
        address: Some(base),
        secret: "ha-token-0123456789".into(),
    };
    setup_queries::store_credential(pool, Service::HomeAssistant, &held)
        .await
        .unwrap();
    requests
}

pub(super) async fn map(
    pool: &PgPool,
    entity: Option<&str>,
    input: Input,
    rain: Option<RainKind>,
) -> Sensor {
    let new = NewSensor {
        name: format!("My {}", input.as_str()),
        entity_id: entity.map(str::to_string),
        input,
        rain_kind: rain,
    };
    let sensor = setup_queries::map_sensor(pool, &new).await.unwrap();
    planning::rebuild(pool).await.unwrap();
    sensor
}

fn asked(requests: &mut UnboundedReceiver<String>) -> usize {
    let mut count = 0;
    while requests.try_recv().is_ok() {
        count += 1;
    }
    count
}

pub(super) fn today() -> chrono::NaiveDate {
    at(0).date_naive()
}

// @spec SYNC-SCHED-013, WX-SRC-HA-001, WX-REC-STORE-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_mapped_sensor_is_read_converted_and_stored_at_its_reported_time(pool: PgPool) {
    located(&pool).await;
    let sensor = map(&pool, Some("sensor.patio"), Input::AirTemp, None).await;
    let mut requests = connect(
        &pool,
        vec![(200, answer("sensor.patio", "20", "°C", at(-2)))],
    )
    .await;

    let read = SensorReader::new()
        .read(&pool, today(), at(0))
        .await
        .unwrap();
    assert_eq!(
        read,
        Read::Done {
            sensors: 1,
            stored: 1,
            announce: true
        }
    );
    assert_eq!(asked(&mut requests), 1);
    let key = sensor.source_key();
    let source = state_of(&pool, &key).await.source_id;
    let (when, value) = sensors::latest_reading(&pool, source, Input::AirTemp)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(when, at(-2));
    assert!((value - 68.0).abs() < 1e-9);
    assert_eq!(state_of(&pool, &key).await.last_success, Some(at(0)));
}

// @spec SYNC-SCHED-013, SYNC-SCHED-017, WX-SRC-HA-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn sensors_are_read_every_five_minutes_and_an_unchanged_time_stores_nothing(pool: PgPool) {
    located(&pool).await;
    map(&pool, Some("sensor.patio"), Input::AirTemp, None).await;
    let same = answer("sensor.patio", "61", "°F", at(-1));
    let mut requests = connect(&pool, vec![(200, same), (200, same)]).await;
    let mut reader = SensorReader::new();

    reader.read(&pool, today(), at(0)).await.unwrap();
    assert_eq!(
        reader.read(&pool, today(), at(4)).await.unwrap(),
        Read::NotDue
    );
    assert_eq!(asked(&mut requests), 1, "no request inside 5 minutes");
    let again = reader.read(&pool, today(), at(5)).await.unwrap();
    assert_eq!(
        again,
        Read::Done {
            sensors: 1,
            stored: 0,
            announce: false
        },
        "the same reported time is nothing new, and only the first read is announced"
    );
    assert_eq!(asked(&mut requests), 1);
}

// @spec SYNC-SCHED-014
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_no_connection_or_no_home_assistant_sensor_nothing_is_asked(pool: PgPool) {
    located(&pool).await;
    map(&pool, Some("sensor.patio"), Input::AirTemp, None).await;
    let mut reader = SensorReader::new();
    assert_eq!(
        reader.read(&pool, today(), at(0)).await.unwrap(),
        Read::NoSensors,
        "no connection stored"
    );

    sqlx::query("UPDATE sensor SET unmapped_at = NOW()")
        .execute(&pool)
        .await
        .unwrap();
    map(&pool, None, Input::Humidity, None).await;
    let mut requests = connect(&pool, vec![(200, "{}")]).await;
    assert_eq!(
        SensorReader::new()
            .read(&pool, today(), at(0))
            .await
            .unwrap(),
        Read::NoSensors,
        "only a posted sensor is mapped"
    );
    assert_eq!(asked(&mut requests), 0);
}

// @spec SYNC-SCHED-015, WX-SRC-HA-002, WX-SRC-HA-008, WX-SRC-HA-013
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_sensor_that_cannot_be_read_does_not_stop_the_others_and_says_why(pool: PgPool) {
    located(&pool).await;
    let gone = map(&pool, Some("sensor.gone"), Input::AirTemp, None).await;
    let kelvin = map(&pool, Some("sensor.kelvin"), Input::SoilTemp10, None).await;
    let good = map(&pool, Some("sensor.good"), Input::Humidity, None).await;
    connect(
        &pool,
        vec![
            (404, r#"{"message":"Entity not found."}"#),
            (200, answer("sensor.kelvin", "285", "K", at(-1))),
            (200, answer("sensor.good", "70", "%", at(-1))),
        ],
    )
    .await;

    let read = SensorReader::new()
        .read(&pool, today(), at(0))
        .await
        .unwrap();
    assert_eq!(
        read,
        Read::Done {
            sensors: 3,
            stored: 1,
            announce: true
        }
    );
    for (sensor, says) in [(&gone, "sensor.gone"), (&kelvin, "K")] {
        let state = state_of(&pool, &sensor.source_key()).await;
        assert_eq!(state.last_failure, Some(at(0)));
        let error = state.last_error.unwrap();
        assert!(error.contains(says), "{error}");
    }
    let state = state_of(&pool, &good.source_key()).await;
    assert_eq!(
        (state.last_success, state.last_failure),
        (Some(at(0)), None)
    );
}

// @spec SYNC-SCHED-015, SYNC-SCHED-016, SYNC-SCHED-017
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_refused_token_fails_every_sensor_and_is_retried_after_five_then_fifteen_minutes(
    pool: PgPool,
) {
    located(&pool).await;
    let air = map(&pool, Some("sensor.air"), Input::AirTemp, None).await;
    let wet = map(&pool, Some("sensor.wet"), Input::Humidity, None).await;
    let refused = (401, r#"{"message":"Invalid access token"}"#);
    let mut requests = connect(
        &pool,
        vec![
            refused,
            refused,
            (200, answer("sensor.air", "61", "°F", at(19))),
            (200, answer("sensor.wet", "70", "%", at(19))),
        ],
    )
    .await;
    let mut reader = SensorReader::new();

    let first = reader.read(&pool, today(), at(0)).await.unwrap();
    assert_eq!(first, Read::Failed(Unusable::Refused));
    for sensor in [&air, &wet] {
        let state = state_of(&pool, &sensor.source_key()).await;
        assert_eq!(state.last_failure, Some(at(0)));
        assert_eq!(
            state.last_error.as_deref(),
            Some(Unusable::Refused.sentence())
        );
    }
    assert_eq!(
        reader.read(&pool, today(), at(5)).await.unwrap(),
        Read::Failed(Unusable::Refused)
    );
    assert_eq!(
        reader.read(&pool, today(), at(15)).await.unwrap(),
        Read::NotDue
    );
    let back = reader.read(&pool, today(), at(20)).await.unwrap();
    assert_eq!(
        back,
        Read::Done {
            sensors: 2,
            stored: 2,
            announce: true
        },
        "the first good read after a failure is announced"
    );
    assert_eq!(asked(&mut requests), 4);
}
