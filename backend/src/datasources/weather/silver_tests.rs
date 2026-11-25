//! Silver reads against small parquet files: the local-day buckets.

use super::fixtures::{self, SilverRow};
use chrono::{Duration, NaiveDate, NaiveDateTime};

fn ts(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
}

fn hour(at: NaiveDateTime, soil_10_c: Option<f64>) -> SilverRow {
    SilverRow {
        at,
        soil_temp_10: soil_10_c,
        air_temp_c: Some(10.0),
        rh_pct: Some(60.0),
        precip_mm: Some(0.0),
        ..Default::default()
    }
}

// @spec WX-SRC-LAKE-003
#[tokio::test]
async fn silver_is_converted_to_fahrenheit_with_sentinels_and_negatives_dropped() {
    let good = SilverRow {
        at: ts("2026-09-20 13:00:00"),
        soil_temp_10: Some(20.0),
        soil_temp_5: Some(-49.0),
        soil_moisture_10: Some(0.25),
        air_temp_c: Some(10.0),
        rh_pct: Some(55.0),
        precip_mm: Some(2.5),
        ..Default::default()
    };
    let bad = SilverRow {
        at: ts("2026-09-20 12:00:00"),
        soil_temp_10: Some(-50.0),
        soil_temp_5: Some(-9999.0),
        soil_moisture_10: Some(-0.1),
        air_temp_c: Some(-60.0),
        rh_pct: Some(-1.0),
        precip_mm: Some(-1.0),
        ..Default::default()
    };
    let file = fixtures::silver(&[good, bad]);
    let rows = fixtures::client(&file.path, "unused.parquet")
        .fetch_range(
            ts("2026-09-20 00:00:00").and_utc(),
            ts("2026-09-21 00:00:00").and_utc(),
        )
        .await
        .unwrap();

    let (newest, oldest) = (&rows[0], &rows[1]);
    assert_eq!(newest.soil_temp_10_f, Some(68.0));
    assert!(
        (newest.soil_temp_5_f.unwrap() - -56.2).abs() < 1e-9,
        "-49 °C kept"
    );
    assert_eq!(newest.ambient_temp_f, Some(50.0));
    assert_eq!(newest.humidity_percent, Some(55.0));
    assert_eq!(newest.soil_moisture_10, Some(0.25));
    assert_eq!(
        newest.precipitation_mm,
        Some(2.5),
        "precipitation stays in mm"
    );

    assert_eq!(oldest.soil_temp_10_f, None, "-50 °C is a sentinel");
    assert_eq!(oldest.soil_temp_5_f, None);
    assert_eq!(oldest.ambient_temp_f, None);
    assert_eq!(oldest.soil_moisture_10, None);
    assert_eq!(oldest.humidity_percent, None);
    assert_eq!(oldest.precipitation_mm, None);
}

// @spec WX-SRC-LAKE-009
#[tokio::test]
async fn dawn_and_afternoon_flags_count_only_usable_hours() {
    // Local = UTC − 4 h. 05:00 local has RH 0 (unusable); 14:00 local is usable.
    let file = fixtures::silver(&[
        SilverRow {
            at: ts("2026-09-20 09:00:00"),
            air_temp_c: Some(15.0),
            rh_pct: Some(0.0),
            ..Default::default()
        },
        SilverRow {
            at: ts("2026-09-20 18:00:00"),
            air_temp_c: Some(25.0),
            rh_pct: Some(50.0),
            ..Default::default()
        },
    ]);
    let lake = fixtures::client(&file.path, "unused.parquet")
        .fetch_daily_disease_inputs(NaiveDate::from_ymd_opt(2026, 9, 1).unwrap())
        .await
        .unwrap();
    let day = &lake.days[0];
    assert_eq!(day.hours_covered, 1.0);
    assert!(!day.covers_dawn, "the only dawn hour has no RH");
    assert!(day.covers_afternoon);
    assert_eq!(day.temp_mean_c, 25.0);
}

// @spec WX-SRC-LAKE-011
#[tokio::test]
async fn daily_reads_use_the_lakes_local_date_as_given() {
    // Twelve hours spanning UTC midnight, all labelled Sep 20 by the lake.
    let label = NaiveDate::from_ymd_opt(2026, 9, 20).unwrap();
    let rows: Vec<SilverRow> = (0..12)
        .map(|h| SilverRow {
            local_date: Some(label),
            soil_temp_5: Some(10.0),
            ..hour(ts("2026-09-20 18:00:00") + Duration::hours(h), None)
        })
        .collect();
    let silver = fixtures::silver(&rows);
    let gold = fixtures::gold(&[fixtures::GoldRow::day(label)]);
    let client = fixtures::client(&silver.path, &gold.path);
    let start = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();

    let disease = client.fetch_daily_disease_inputs(start).await.unwrap();
    assert_eq!(disease.days.len(), 1);
    assert_eq!(
        (disease.days[0].date, disease.days[0].hours_covered),
        (label, 12.0)
    );

    let climate = client.fetch_climate_record(start).await.unwrap();
    assert_eq!(climate.days.len(), 1);
    assert_eq!(climate.days[0].date, label);
    assert_eq!(climate.days[0].soil_temp_5_f, Some(50.0));
}
