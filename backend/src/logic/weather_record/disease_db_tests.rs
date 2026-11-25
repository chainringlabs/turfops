//! The disease models' daily inputs against PostgreSQL.

use super::lab::*;
use crate::db::weather_queries::days;
use crate::models::weather_record::{DailyRow, HourlyRow, Input};
use crate::models::{AFTERNOON_HOURS, DAWN_HOURS};
use chrono::Duration;
use chrono_tz::America::New_York;
use sqlx::PgPool;

async fn inputs(lab: &Lab, start: u32, today: u32) -> super::disease::DiseaseInputs {
    lab.record
        .fetch_daily_disease_inputs(d(start), DAWN_HOURS, AFTERNOON_HOURS, noon(today))
        .await
        .unwrap()
}

// @spec WX-REC-API-010, DIS-MODEL-DAY-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_day_is_its_usable_hours_in_celsius_with_humid_and_wet_hours(pool: PgPool) {
    let lab = Lab::new(pool).await;
    let start = days::start_of_day(New_York, d(18));
    // 24 hours at 68 °F: twelve dry at 50% and twelve at 95%, three of the dry ones wet
    // with rain; one more hour has no humidity and is not usable.
    let mut rows: Vec<HourlyRow> = (0..24)
        .map(|hour| HourlyRow {
            observed_at: start + Duration::hours(hour),
            air_temp_f: Some(68.0),
            air_temp_max_f: Some(if hour == 15 { 86.0 } else { 68.0 }),
            air_temp_min_f: Some(if hour == 5 { 50.0 } else { 68.0 }),
            rh_pct: Some(if hour < 12 { 50.0 } else { 95.0 }),
            precip_mm: Some(if hour < 3 { 2.0 } else { 0.0 }),
            ..Default::default()
        })
        .collect();
    rows[23].rh_pct = None;
    lab.write_hours(ANALYSIS, &rows).await;
    let got = inputs(&lab, 18, 20).await;
    assert_eq!(got.days.len(), 1);
    let day = &got.days[0];
    assert_eq!((day.date, day.hours_covered), (d(18), 23.0));
    assert!((day.temp_mean_c - 20.0).abs() < 1e-9);
    assert!((day.temp_min_c - 10.0).abs() < 1e-9 && (day.temp_max_c - 30.0).abs() < 1e-9);
    assert_eq!((day.hours_rh90, day.leaf_wetness_hours), (11.0, 14.0));
    assert!((day.rh_mean - (12.0 * 50.0 + 11.0 * 95.0) / 23.0).abs() < 1e-9);
    // Saturated air's dew point is its temperature; at 95% it is just under 20 °C.
    assert!(
        (day.dew_point_max_c - 19.2).abs() < 0.1,
        "{}",
        day.dew_point_max_c
    );
    assert!(day.covers_dawn && day.covers_afternoon && !day.is_forecast);
    assert_eq!(got.latest_observed, Some(start + Duration::hours(22)));
    assert_eq!(got.basis.len(), 1);
    assert!(got.basis[0].basis.approximate);
    assert_eq!(
        day.basis.as_ref(),
        Some(&got.basis[0].basis),
        "the day's own basis"
    );
}

// @spec WX-REC-API-010
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_days_rain_total_comes_from_the_rain_basis(pool: PgPool) {
    let lab = Lab::new(pool).await;
    lab.analysis_hours(17, 24, 70.0, 60.0, 0.25).await;
    lab.analysis_hours(18, 24, 70.0, 60.0, 0.25).await;
    // PRISM's day 18 is the lawn's 17th.
    let rain = DailyRow {
        day: d(18),
        precip_mm: Some(11.0),
        ..Default::default()
    };
    lab.daily(PRISM, rain).await;
    let got = inputs(&lab, 17, 20).await;
    let totals: Vec<_> = got
        .days
        .iter()
        .map(|day| (day.date, day.precip_mm))
        .collect();
    assert_eq!(totals.len(), 2);
    assert_eq!(totals[0].0, d(17));
    assert!((totals[0].1 - 11.0).abs() < 1e-9, "PRISM: {}", totals[0].1);
    assert!(
        (totals[1].1 - 6.0).abs() < 1e-9,
        "the analysis until PRISM publishes"
    );
}

// @spec WX-REC-RES-005, WX-REC-RES-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_day_needs_twelve_hours_of_both_and_the_day_in_progress_does_not(pool: PgPool) {
    let lab = Lab::new(pool).await;
    lab.analysis_hours(17, 11, 70.0, 60.0, 0.0).await;
    lab.analysis_hours(18, 12, 70.0, 60.0, 0.0).await;
    lab.analysis_hours(20, 11, 70.0, 60.0, 0.0).await;
    let got = inputs(&lab, 17, 20).await;
    let days: Vec<_> = got
        .days
        .iter()
        .map(|day| (day.date, day.hours_covered))
        .collect();
    assert_eq!(days, vec![(d(18), 12.0), (d(20), 11.0)]);
    assert!(got.days[1].covers_dawn && !got.days[1].covers_afternoon);
}

// @spec WX-REC-RES-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_sensor_that_measures_only_temperature_does_not_supply_disease_days(pool: PgPool) {
    let lab = Lab::new(pool).await;
    let sensor = lab.map_sensor(Input::AirTemp).await;
    let start = days::start_of_day(New_York, d(18));
    let rows: Vec<HourlyRow> = (0..24)
        .map(|hour| HourlyRow {
            observed_at: start + Duration::hours(hour),
            air_temp_f: Some(99.0),
            ..Default::default()
        })
        .collect();
    lab.write_hours(&sensor.key, &rows).await;
    lab.analysis_hours(18, 24, 68.0, 60.0, 0.0).await;
    let got = inputs(&lab, 18, 20).await;
    assert_eq!(got.days.len(), 1);
    assert!(
        (got.days[0].temp_mean_c - 20.0).abs() < 1e-9,
        "the analysis has both"
    );
    assert_eq!(got.basis[0].basis.source, "Open-Meteo hourly analysis");
}
