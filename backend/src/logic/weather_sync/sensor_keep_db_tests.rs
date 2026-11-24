//! What the owner's sensors keep between reads, and how long their readings are kept,
//! against PostgreSQL and a stubbed Home Assistant.

use super::schedule::Schedule;
use super::schedule_db_tests::{at, located, state_of, unreachable};
use super::sensor_read::{Read, SensorReader};
use super::sensor_read_db_tests::{answer, connect, map, today};
use crate::db::setup_queries::{self, RainKind, Sensor};
use crate::db::weather_queries::sensors;
use crate::models::weather_record::Input;
use chrono::Duration;
use sqlx::PgPool;

// @spec WX-SRC-HA-006, WX-SRC-HA-009, WX-SRC-HA-011
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_rain_counter_keeps_its_starting_point_across_a_restart(pool: PgPool) {
    located(&pool).await;
    let gauge = map(
        &pool,
        Some("sensor.gauge"),
        Input::Rain,
        Some(RainKind::Counter),
    )
    .await;
    connect(
        &pool,
        vec![
            (200, answer("sensor.gauge", "10.0", "in", at(-1))),
            (200, answer("sensor.gauge", "10.1", "in", at(4))),
        ],
    )
    .await;

    let first = SensorReader::new()
        .read(&pool, today(), at(0))
        .await
        .unwrap();
    assert_eq!(
        first,
        Read::Done {
            sensors: 1,
            stored: 0,
            announce: true
        },
        "the first reading is the starting point"
    );
    // A new reader is a restart: the starting point comes from the database.
    SensorReader::new()
        .read(&pool, today(), at(5))
        .await
        .unwrap();
    let source = state_of(&pool, &gauge.source_key()).await.source_id;
    let (when, amount) = sensors::latest_reading(&pool, source, Input::Rain)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(when, at(4));
    assert!((amount - 2.54).abs() < 1e-9, "{amount}");
    let kept = sensors::previous_reading(&pool, gauge.id)
        .await
        .unwrap()
        .unwrap();
    assert!((kept.0 - 256.54).abs() < 1e-9);
    assert_eq!(kept.1, at(4));
}

// @spec WX-SRC-HA-012
#[sqlx::test(migrations = "src/db/migrations")]
async fn changing_what_a_sensor_measures_discards_its_previous_reading(pool: PgPool) {
    located(&pool).await;
    let probe = map(&pool, Some("sensor.probe"), Input::SoilTemp5, None).await;
    sensors::keep_reading(&pool, probe.id, (50.0, at(-10)), None)
        .await
        .unwrap();
    setup_queries::change_sensor(&pool, probe.id, "Renamed", Input::SoilTemp5, None)
        .await
        .unwrap();
    assert!(
        sensors::previous_reading(&pool, probe.id)
            .await
            .unwrap()
            .is_some(),
        "a new name keeps it"
    );
    setup_queries::change_sensor(&pool, probe.id, "Renamed", Input::SoilTemp10, None)
        .await
        .unwrap();
    assert_eq!(
        sensors::previous_reading(&pool, probe.id).await.unwrap(),
        None
    );
}

// @spec SYNC-SCHED-018, WX-REC-STORE-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn sensor_readings_older_than_a_week_are_deleted_once_a_day(pool: PgPool) {
    located(&pool).await;
    let probe = map(&pool, None, Input::AirTemp, None).await;
    let source = sources_id(&pool, &probe).await;
    for days in [8, 1] {
        sensors::record_reading(
            &pool,
            source,
            Input::AirTemp,
            at(0) - Duration::days(days),
            60.0,
        )
        .await
        .unwrap();
    }
    let mut reader = SensorReader::new();
    assert_eq!(reader.prune(&pool, today(), at(0)).await.unwrap(), Some(1));
    sensors::record_reading(
        &pool,
        source,
        Input::AirTemp,
        at(0) - Duration::days(9),
        60.0,
    )
    .await
    .unwrap();
    assert_eq!(reader.prune(&pool, today(), at(60)).await.unwrap(), None);
    let tomorrow = today() + Duration::days(1);
    assert_eq!(
        reader.prune(&pool, tomorrow, at(24 * 60)).await.unwrap(),
        Some(1)
    );
}

async fn sources_id(pool: &PgPool, sensor: &Sensor) -> i64 {
    crate::db::weather_queries::sources::all(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.key == sensor.source_key())
        .unwrap()
        .id
}

// @spec SYNC-SCHED-003, SYNC-SCHED-013
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_schedule_pass_reads_the_sensors(pool: PgPool) {
    located(&pool).await;
    let sensor = map(&pool, Some("sensor.patio"), Input::AirTemp, None).await;
    connect(
        &pool,
        vec![(200, answer("sensor.patio", "61", "°F", at(-1)))],
    )
    .await;
    let (mut schedule, _, _) = Schedule::new(pool.clone(), unreachable());
    schedule.pass(at(0)).await.unwrap();
    let state = state_of(&pool, &sensor.source_key()).await;
    assert_eq!(state.last_success, Some(at(0)));
}
