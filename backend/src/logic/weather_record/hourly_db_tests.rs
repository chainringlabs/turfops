//! Hourly ranges and the summary against PostgreSQL.

use super::lab::*;
use crate::db::weather_queries::days;
use crate::models::weather_record::{DailyRow, Input};
use crate::models::Trend;
use chrono::Duration;
use chrono_tz::America::New_York;
use sqlx::PgPool;

/// The estimate has the 17th and the station the whole of the 18th; the analysis has
/// both days and this morning. PRISM's rows for the 14th and 18th are the rain of the
/// lawn's 13th and 17th.
async fn week(pool: PgPool) -> Lab {
    let lab = Lab::new(pool).await;
    lab.estimate(17, 60.0).await;
    lab.station_hours(18, 24, 70.0).await;
    for day in 17..=19 {
        lab.analysis_hours(day, 24, 80.0, 50.0, 0.0).await;
    }
    lab.analysis_hours(20, 11, 90.0, 40.0, 0.5).await;
    for day in [14, 18] {
        let rain = DailyRow {
            day: d(day),
            precip_mm: Some(24.0),
            ..Default::default()
        };
        lab.daily(PRISM, rain).await;
    }
    lab
}

// @spec WX-REC-API-002, WX-REC-BASIS-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_range_is_hourly_newest_first_with_a_daily_basis_day_as_one_reading_at_noon(
    pool: PgPool,
) {
    let lab = week(pool).await;
    let start = days::start_of_day(New_York, d(17));
    let end = days::start_of_day(New_York, d(19)) - Duration::hours(1);
    let series = lab.record.fetch_range(start, end, noon(20)).await.unwrap();
    assert_eq!(series.rows.len(), 48);
    assert_eq!(series.rows[0].timestamp, end, "newest first");
    assert_eq!(series.rows[47].timestamp, start);
    assert!(series.rows.iter().all(|r| r.ambient_temp_f == Some(80.0)));

    let soil: Vec<_> = series
        .rows
        .iter()
        .filter(|r| r.soil_temp_10_f.is_some())
        .collect();
    assert_eq!(
        soil.len(),
        25,
        "24 station hours and the estimate's one reading"
    );
    let estimate = soil.last().unwrap();
    assert_eq!(estimate.soil_temp_10_f, Some(60.0));
    assert_eq!(
        estimate.timestamp,
        start + Duration::hours(12),
        "local noon on the 17th"
    );
    assert_eq!(estimate.soil_temp_5_f, Some(62.0));

    // PRISM's rain for the lawn's 17th is one reading; the 18th falls to the analysis.
    let rain: Vec<_> = series
        .rows
        .iter()
        .filter_map(|r| r.precipitation_mm)
        .collect();
    assert_eq!(rain.iter().filter(|mm| **mm == 24.0).count(), 1);
    assert_eq!(rain.len(), 25);

    let spans: Vec<_> = series
        .basis
        .iter()
        .filter(|s| s.input == Input::SoilTemp10)
        .map(|s| (s.from, s.to, s.basis.source.as_str()))
        .collect();
    assert_eq!(
        spans,
        vec![
            (d(17), d(17), "Soil temperature estimate"),
            (d(18), d(18), "Avondale 2 N, PA (USCRN)"),
        ]
    );
}

// @spec WX-REC-API-002, WX-REC-RES-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_day_in_progress_is_in_a_range_from_the_source_that_is_reporting(pool: PgPool) {
    let lab = week(pool).await;
    let start = days::start_of_day(New_York, d(20));
    let series = lab
        .record
        .fetch_range(start, noon(20).at.now, noon(20))
        .await
        .unwrap();
    assert_eq!(series.rows.len(), 11);
    assert!(series.rows.iter().all(|r| r.ambient_temp_f == Some(90.0)));
    assert!(
        series.rows.iter().all(|r| r.soil_temp_10_f.is_none()),
        "the station has not reached today"
    );
}

// @spec WX-REC-API-003, WX-REC-BASIS-004, WX-REC-FAIL-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_summary_is_the_168_hours_ending_now(pool: PgPool) {
    let lab = week(pool).await;
    let summary = lab.record.fetch_summary(noon(20)).await.unwrap();

    // 24 hours of the estimate at 60 and 24 station hours at 70.
    let soil = summary.soil_temp_7day_avg_f.unwrap();
    assert_eq!(soil.value, 65.0);
    assert_eq!(soil.basis.source, "Avondale 2 N, PA (USCRN)");
    assert!(soil.mixed);

    // 72 hours at 80 and this morning's 11 at 90, all from the analysis.
    let air = summary.ambient_temp_7day_avg_f.unwrap();
    assert!((air.value - (72.0 * 80.0 + 11.0 * 90.0) / 83.0).abs() < 1e-9);
    assert!(!air.mixed && air.basis.approximate);
    assert!(
        (summary.humidity_7day_avg.unwrap().value - (72.0 * 50.0 + 11.0 * 40.0) / 83.0).abs()
            < 1e-9
    );

    // The window opens at 1 pm on the 13th: 11 of that day's 24 hours of PRISM's 24 mm,
    // all of the 17th's, and this morning's 11 analysis hours at 0.5 mm.
    let rain = summary.precipitation_7day_total_mm.unwrap();
    assert!(
        (rain.value - (11.0 + 24.0 + 5.5)).abs() < 1e-9,
        "{}",
        rain.value
    );
    assert!(rain.mixed);

    assert_eq!(summary.live.get(Input::AirTemp).unwrap().value, 90.0);
    assert_eq!(summary.live.get(Input::SoilTemp10).unwrap().value, 70.0);
    assert_eq!(
        summary.soil_temp_trend,
        Trend::Unknown,
        "the day before rests on the estimate"
    );
}

// @spec WX-REC-API-003, WX-REC-FAIL-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_figure_with_no_hours_behind_it_is_empty_not_zero(pool: PgPool) {
    let lab = Lab::new(pool).await;
    let summary = lab.record.fetch_summary(noon(20)).await.unwrap();
    assert_eq!(summary.soil_temp_7day_avg_f, None);
    assert_eq!(summary.precipitation_7day_total_mm, None);
    assert_eq!(summary.soil_temp_trend, Trend::Unknown);
    assert!(summary.live.values.is_empty());
}

// @spec WX-REC-API-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_trend_compares_the_newest_24_hours_with_the_24_before(pool: PgPool) {
    let lab = Lab::new(pool).await;
    lab.station_hours(17, 24, 60.0).await;
    lab.station_hours(18, 24, 62.5).await;
    let summary = lab.record.fetch_summary(noon(20)).await.unwrap();
    assert_eq!(summary.soil_temp_trend, Trend::Rising);
}
