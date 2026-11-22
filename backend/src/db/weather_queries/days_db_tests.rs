//! An hourly source's local days against PostgreSQL.

use super::{days, rows, sources};
use crate::models::weather_record::*;
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::America::New_York;
use sqlx::PgPool;

async fn station(pool: &PgPool) -> i64 {
    let source = NewSource {
        key: "station:Uscrn:a".into(),
        kind: SourceKind::Station,
        name: "A".into(),
        station_ref: None,
        latitude: None,
        longitude: None,
        distance_miles: Some(5.0),
        role: SourceRole::Basis,
    };
    sources::ensure(pool, &source).await.unwrap().id
}

fn utc(day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, day, hour, 0, 0).unwrap()
}

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 7, day).unwrap()
}

async fn write(pool: &PgPool, source_id: i64, hours: &[HourlyRow]) {
    let mut conn = pool.acquire().await.unwrap();
    rows::write_hourly(&mut conn, source_id, hours)
        .await
        .unwrap();
}

// @spec WX-REC-STORE-005, WX-REC-RES-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn hours_are_grouped_into_the_lawns_days_not_utc_days(pool: PgPool) {
    let id = station(&pool).await;
    // 04:00 UTC on the 15th is midnight Eastern; 03:00 UTC is 11 pm on the 14th.
    let hours: Vec<HourlyRow> = (3..=6)
        .map(|hour| HourlyRow {
            observed_at: utc(15, hour),
            air_temp_f: Some(60.0 + f64::from(hour)),
            precip_mm: Some(1.0),
            ..Default::default()
        })
        .collect();
    write(&pool, id, &hours).await;
    let got = days::local_days(&pool, id, New_York, d(14), d(15))
        .await
        .unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(
        (got[0].day, got[0].air.hours, got[0].air.mean),
        (d(14), 1, Some(63.0))
    );
    assert_eq!(
        (got[1].day, got[1].air.hours, got[1].air.mean),
        (d(15), 3, Some(65.0))
    );
    assert_eq!((got[1].rain_hours, got[1].rain_mm), (3, Some(3.0)));
    let only = days::local_days(&pool, id, New_York, d(15), d(15))
        .await
        .unwrap();
    assert_eq!(
        only.len(),
        1,
        "the 14th's hour is outside the span asked for"
    );
}

// @spec WX-REC-STORE-005, WX-REC-FAIL-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn each_value_counts_its_own_valid_hours(pool: PgPool) {
    let id = station(&pool).await;
    let hours: Vec<HourlyRow> = (0..10)
        .map(|i| {
            let mut row = HourlyRow {
                observed_at: utc(15, 10) + Duration::hours(i),
                air_temp_f: Some(70.0 + i as f64),
                air_temp_max_f: Some(72.0 + i as f64),
                rh_pct: Some(50.0),
                ..Default::default()
            };
            if i < 4 {
                row.soil_temp_f[1] = Some(64.0);
                row.soil_moisture[4] = Some(0.3);
            }
            row
        })
        .collect();
    write(&pool, id, &hours).await;
    let day = &days::local_days(&pool, id, New_York, d(15), d(15))
        .await
        .unwrap()[0];
    assert_eq!((day.air.hours, day.humidity.hours), (10, 10));
    assert_eq!((day.air_max, day.air_min), (Some(81.0), Some(70.0)));
    assert_eq!(
        day.soil_temp[1],
        days::Stat {
            hours: 4,
            mean: Some(64.0)
        }
    );
    assert_eq!(
        day.soil_temp[0],
        days::Stat {
            hours: 0,
            mean: None
        }
    );
    assert_eq!(day.soil_moisture[4].hours, 4);
    assert_eq!(
        (day.rain_hours, day.rain_mm),
        (0, None),
        "no rain reported is not zero"
    );
}

// @spec WX-REC-RES-010
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_newest_hour_of_each_value_is_found_on_its_own(pool: PgPool) {
    let id = station(&pool).await;
    let mut early = HourlyRow {
        observed_at: utc(15, 10),
        air_temp_f: Some(70.0),
        ..Default::default()
    };
    early.soil_temp_f[1] = Some(64.0);
    let late = HourlyRow {
        observed_at: utc(15, 14),
        air_temp_f: Some(75.0),
        ..Default::default()
    };
    write(&pool, id, &[early, late]).await;
    let newest = days::newest_hours(&pool, id, utc(14, 0)).await.unwrap();
    assert_eq!(newest.air, Some(utc(15, 14)));
    assert_eq!(
        newest.soil_temp[1],
        Some(utc(15, 10)),
        "the probe skipped the last hours"
    );
    assert_eq!((newest.humidity, newest.soil_temp[0]), (None, None));
    let recent = days::newest_hours(&pool, id, utc(15, 12)).await.unwrap();
    assert_eq!(recent.soil_temp[1], None, "older than the look-back");
}

// @spec WX-REC-STORE-005
#[test]
fn a_local_day_starts_at_local_midnight_through_a_clock_change() {
    let day = |m, d| NaiveDate::from_ymd_opt(2026, m, d).unwrap();
    let start = |m, d| days::start_of_day(New_York, day(m, d));
    assert_eq!(start(7, 15), utc(15, 4));
    // Clocks go back on 2026-11-01: that day is 25 hours long.
    assert_eq!(start(11, 2) - start(11, 1), Duration::hours(25));
}
