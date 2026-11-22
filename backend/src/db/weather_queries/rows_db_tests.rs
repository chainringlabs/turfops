//! Hourly rows, daily rows and sensor readings against PostgreSQL.

use super::{rows, sensors, sources};
use crate::models::weather_record::*;
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use sqlx::PgPool;

fn at(day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, day, hour, minute, 0).unwrap()
}

fn new_source(key: &str, kind: SourceKind, role: SourceRole) -> NewSource {
    NewSource {
        key: key.into(),
        kind,
        name: key.into(),
        station_ref: None,
        latitude: Some(39.86),
        longitude: Some(-75.79),
        distance_miles: None,
        role,
    }
}

async fn source(pool: &PgPool, key: &str, kind: SourceKind) -> Source {
    sources::ensure(pool, &new_source(key, kind, SourceRole::Basis))
        .await
        .unwrap()
}

fn hour(observed_at: DateTime<Utc>, air: Option<f64>, soil10: Option<f64>) -> HourlyRow {
    HourlyRow {
        observed_at,
        air_temp_f: air,
        soil_temp_f: [None, soil10, None, None, None],
        ..Default::default()
    }
}

// @spec WX-REC-STORE-001, WX-REC-STORE-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn hourly_rows_are_kept_per_source_as_handed_over(pool: PgPool) {
    let station = source(&pool, "station:Uscrn:3761", SourceKind::Station).await;
    let analysis = source(&pool, "analysis:39.86,-75.79", SourceKind::Analysis).await;
    let full = HourlyRow {
        observed_at: at(1, 5, 0),
        air_temp_f: Some(61.5),
        air_temp_max_f: Some(63.0),
        air_temp_min_f: Some(60.1),
        rh_pct: Some(88.0),
        precip_mm: Some(1.2),
        soil_temp_f: [Some(64.0), Some(63.5), Some(62.0), Some(60.0), Some(57.5)],
        soil_moisture: [Some(0.21), Some(0.25), Some(0.3), Some(0.33), Some(0.35)],
    };
    let mut conn = pool.acquire().await.unwrap();
    rows::write_hourly(&mut conn, station.id, std::slice::from_ref(&full))
        .await
        .unwrap();
    rows::write_hourly(
        &mut conn,
        analysis.id,
        &[hour(at(1, 5, 0), Some(59.0), None)],
    )
    .await
    .unwrap();
    let from_station = rows::hourly(&pool, station.id, at(1, 0, 0), at(2, 0, 0))
        .await
        .unwrap();
    assert_eq!(
        from_station,
        vec![full],
        "no unit is converted and no source is mixed in"
    );
    let from_analysis = rows::hourly(&pool, analysis.id, at(1, 0, 0), at(2, 0, 0))
        .await
        .unwrap();
    assert_eq!(from_analysis[0].air_temp_f, Some(59.0));
}

// @spec WX-REC-STORE-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_rewritten_hour_replaces_the_held_row_including_values_now_empty(pool: PgPool) {
    let station = source(&pool, "station:Uscrn:3761", SourceKind::Station).await;
    let mut conn = pool.acquire().await.unwrap();
    rows::write_hourly(
        &mut conn,
        station.id,
        &[hour(at(1, 5, 0), Some(61.5), Some(63.5))],
    )
    .await
    .unwrap();
    // The network's quality control withdrew the soil value and revised the air.
    rows::write_hourly(
        &mut conn,
        station.id,
        &[hour(at(1, 5, 0), Some(61.9), None)],
    )
    .await
    .unwrap();
    let held = rows::hourly(&pool, station.id, at(1, 0, 0), at(2, 0, 0))
        .await
        .unwrap();
    assert_eq!(held, vec![hour(at(1, 5, 0), Some(61.9), None)]);
}

// @spec WX-REC-STORE-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn an_hourly_row_must_be_stamped_on_the_utc_hour(pool: PgPool) {
    let station = source(&pool, "station:Uscrn:3761", SourceKind::Station).await;
    let mut conn = pool.acquire().await.unwrap();
    let off_the_hour = hour(at(1, 5, 30), Some(61.5), None);
    assert!(rows::write_hourly(&mut conn, station.id, &[off_the_hour])
        .await
        .is_err());
}

fn day(date: NaiveDate, max: f64, min: Option<f64>) -> DailyRow {
    DailyRow {
        day: date,
        air_max_f: Some(max),
        air_min_f: min,
        ..Default::default()
    }
}

// @spec WX-REC-STORE-003, WX-REC-STORE-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn daily_rows_keep_the_sources_own_day_and_are_replaced_when_rewritten(pool: PgPool) {
    let grid = source(&pool, "grid:Prism:39.86,-75.79", SourceKind::Grid).await;
    let june_1 = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
    let mut conn = pool.acquire().await.unwrap();
    rows::write_daily(&mut conn, &grid, &[day(june_1, 81.0, Some(60.0))])
        .await
        .unwrap();
    rows::write_daily(&mut conn, &grid, &[day(june_1, 82.5, None)])
        .await
        .unwrap();
    let held = rows::daily(&pool, grid.id, june_1, june_1).await.unwrap();
    assert_eq!(held, vec![day(june_1, 82.5, None)]);
}

// @spec WX-REC-STORE-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn only_a_source_that_publishes_daily_values_may_hold_daily_rows(pool: PgPool) {
    let june_1 = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
    let rows_in = [day(june_1, 81.0, Some(60.0))];
    let mut conn = pool.acquire().await.unwrap();
    for (key, kind, role, allowed) in [
        ("grid:Prism:1", SourceKind::Grid, SourceRole::Basis, true),
        ("estimate:5", SourceKind::Estimate, SourceRole::Basis, true),
        (
            "station:Uscrn:1",
            SourceKind::Station,
            SourceRole::Calibration,
            true,
        ),
        (
            "station:Uscrn:2",
            SourceKind::Station,
            SourceRole::Basis,
            false,
        ),
        ("analysis:1", SourceKind::Analysis, SourceRole::Basis, false),
        ("sensor:1", SourceKind::Sensor, SourceRole::Basis, false),
    ] {
        let source = sources::ensure(&pool, &new_source(key, kind, role))
            .await
            .unwrap();
        let written = rows::write_daily(&mut conn, &source, &rows_in).await;
        assert_eq!(written.is_ok(), allowed, "{key}");
    }
}

// @spec WX-REC-STORE-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn sensor_readings_fold_into_the_hour_as_a_mean_and_rain_as_a_total(pool: PgPool) {
    let probe = source(&pool, "sensor:1", SourceKind::Sensor).await;
    let gauge = source(&pool, "sensor:2", SourceKind::Sensor).await;
    for (minute, value) in [(5, 60.0), (25, 62.0), (45, 64.0)] {
        sensors::record_reading(&pool, probe.id, Input::SoilTemp10, at(1, 14, minute), value)
            .await
            .unwrap();
    }
    for (minute, value) in [(10, 0.5), (40, 1.25)] {
        sensors::record_reading(&pool, gauge.id, Input::Rain, at(1, 14, minute), value)
            .await
            .unwrap();
    }
    // The next hour is its own row.
    sensors::record_reading(&pool, probe.id, Input::SoilTemp10, at(1, 15, 5), 70.0)
        .await
        .unwrap();

    let soil = rows::hourly(&pool, probe.id, at(1, 0, 0), at(2, 0, 0))
        .await
        .unwrap();
    assert_eq!(soil.len(), 2);
    assert_eq!(
        (soil[0].observed_at, soil[0].soil_temp_f[1]),
        (at(1, 14, 0), Some(62.0))
    );
    assert_eq!(soil[1].soil_temp_f[1], Some(70.0));
    let rain = rows::hourly(&pool, gauge.id, at(1, 0, 0), at(2, 0, 0))
        .await
        .unwrap();
    assert_eq!(rain[0].precip_mm, Some(1.75));
}

// @spec WX-REC-STORE-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_reading_sent_again_for_the_same_instant_replaces_itself(pool: PgPool) {
    let gauge = source(&pool, "sensor:2", SourceKind::Sensor).await;
    for value in [0.5, 0.5, 0.75] {
        sensors::record_reading(&pool, gauge.id, Input::Rain, at(1, 14, 10), value)
            .await
            .unwrap();
    }
    let rain = rows::hourly(&pool, gauge.id, at(1, 0, 0), at(2, 0, 0))
        .await
        .unwrap();
    assert_eq!(
        rain[0].precip_mm,
        Some(0.75),
        "a resend is not counted twice"
    );
    let latest = sensors::latest_reading(&pool, gauge.id, Input::Rain)
        .await
        .unwrap();
    assert_eq!(latest, Some((at(1, 14, 10), 0.75)));
}

// @spec WX-REC-STORE-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn raw_readings_are_dropped_after_seven_days_and_their_hours_are_kept(pool: PgPool) {
    let probe = source(&pool, "sensor:1", SourceKind::Sensor).await;
    sensors::record_reading(&pool, probe.id, Input::AirTemp, at(1, 14, 5), 71.0)
        .await
        .unwrap();
    sensors::record_reading(&pool, probe.id, Input::AirTemp, at(8, 13, 5), 75.0)
        .await
        .unwrap();
    let now = at(8, 14, 6);
    assert_eq!(sensors::prune_readings(&pool, now).await.unwrap(), 1);
    let latest = sensors::latest_reading(&pool, probe.id, Input::AirTemp)
        .await
        .unwrap();
    assert_eq!(latest, Some((at(8, 13, 5), 75.0)));
    let hours = rows::hourly(&pool, probe.id, at(1, 0, 0), now + Duration::hours(1))
        .await
        .unwrap();
    assert_eq!(
        hours.len(),
        2,
        "the hourly row from the dropped reading stays"
    );
    assert_eq!(sensors::READINGS_KEPT, Duration::days(7));
}

// @spec WX-REC-STORE-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn daily_air_is_not_something_a_sensor_reports(pool: PgPool) {
    let probe = source(&pool, "sensor:1", SourceKind::Sensor).await;
    let refused =
        sensors::record_reading(&pool, probe.id, Input::DailyAir, at(1, 14, 5), 71.0).await;
    assert!(refused.is_err());
}
