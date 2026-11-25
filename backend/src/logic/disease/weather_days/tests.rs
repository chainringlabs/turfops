//! Forecast days and the joined series.

use super::*;
use crate::logic::disease::test_support::{date, day};
use crate::models::WeatherCondition;
use chrono::{TimeZone, Utc};

fn point(day: u32, hour: u32, temp_f: f64, rh: f64, precip: f64) -> ForecastPoint {
    ForecastPoint {
        timestamp: Utc.with_ymd_and_hms(2026, 9, day, hour, 0, 0).unwrap(),
        slot_hours: 3,
        temp_f,
        feels_like_f: Some(temp_f),
        humidity_percent: rh,
        precipitation_mm: Some(precip),
        precipitation_prob: Some(0.0),
        wind_speed_mph: 0.0,
        wind_gust_mph: None,
        cloud_cover_percent: Some(0.0),
        weather_condition: WeatherCondition::Clear,
    }
}

#[test]
fn dew_point_matches_known_values() {
    // Saturated air: dew point equals air temp.
    assert!((dew_point_c(20.0, 100.0) - 20.0).abs() < 0.01);
    // 25°C at 60% RH → ~16.7°C.
    assert!((dew_point_c(25.0, 60.0) - 16.7).abs() < 0.1);
}

fn eastern() -> Clock {
    Clock::Zone(chrono_tz::America::New_York)
}

// @spec DIS-MODEL-DAY-003, DIS-MODEL-DAY-004, DIS-MODEL-DAY-005
#[test]
fn forecast_points_bucket_by_local_date() {
    // 02:00 UTC on the 21st is 22:00 on the 20th at UTC-4.
    let points = [
        point(21, 2, 68.0, 95.0, 0.0),
        point(21, 12, 77.0, 60.0, 1.0),
    ];
    let days = forecast_days(&points, &eastern(), None);
    assert_eq!(days.len(), 2);
    assert_eq!(days[0].date, date(9, 20));
    assert_eq!(days[0].hours_rh90, 3.0);
    assert!((days[0].temp_mean_c - 20.0).abs() < 1e-9);
    assert_eq!(days[1].date, date(9, 21));
    assert_eq!(days[1].hours_rh90, 0.0);
    assert_eq!(days[1].leaf_wetness_hours, 3.0);
    assert!(days[1].is_forecast);
    // 22:00 local is neither dawn nor afternoon; 08:00 local (12:00 UTC) is dawn.
    assert!(!days[0].covers_dawn && !days[0].covers_afternoon);
    assert!(days[1].covers_dawn && !days[1].covers_afternoon);
}

// @spec DIS-MODEL-DAY-005
#[test]
fn each_point_counts_as_its_slots_length() {
    // Three humid hourly slots and one dry 3-hour slot, all on Sep 21 local.
    let mut points: Vec<_> = (12..15).map(|h| point(21, h, 68.0, 95.0, 0.0)).collect();
    points.push(point(21, 15, 86.0, 40.0, 0.0));
    for p in &mut points[..3] {
        p.slot_hours = 1;
    }
    let day = &forecast_days(&points, &eastern(), None)[0];
    assert_eq!(day.hours_covered, 6.0);
    assert_eq!(day.hours_rh90, 3.0);
    assert!((day.rh_mean - (3.0 * 95.0 + 3.0 * 40.0) / 6.0).abs() < 1e-9);
    assert!((day.temp_mean_c - (20.0 * 3.0 + 30.0 * 3.0) / 6.0).abs() < 1e-9);
}

// @spec DIS-MODEL-DAY-005
#[test]
fn a_point_with_no_rain_amount_adds_no_wet_hours_and_no_rain() {
    let mut points = [
        point(21, 12, 77.0, 60.0, 1.0),
        point(21, 15, 77.0, 60.0, 0.0),
        point(21, 18, 77.0, 95.0, 0.0),
    ];
    points[1].precipitation_mm = None;
    points[2].precipitation_mm = None;
    let day = &forecast_days(&points, &eastern(), None)[0];
    assert_eq!(
        day.leaf_wetness_hours, 6.0,
        "the rainy slot and the humid one"
    );
    assert_eq!(day.precip_mm, 1.0);
}

// @spec DIS-MODEL-DAY-003
#[test]
fn forecast_points_follow_daylight_saving() {
    // DST ends 2026-11-01 06:00 UTC. 04:00 UTC Nov 2 is 11 pm EST Nov 1 (a fixed EDT
    // offset would say midnight Nov 2); 12:00 UTC Nov 2 is 7 am EST, a dawn hour.
    let at = |d: u32, h: u32| {
        let mut p = point(1, 0, 50.0, 50.0, 0.0);
        p.timestamp = Utc.with_ymd_and_hms(2026, 11, d, h, 0, 0).unwrap();
        p
    };
    let days = forecast_days(&[at(2, 4), at(2, 12)], &eastern(), None);
    assert_eq!(days.len(), 2);
    assert_eq!(
        days[0].date,
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()
    );
    assert_eq!(
        days[1].date,
        chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()
    );
    assert!(days[1].covers_dawn);
}

// @spec DIS-MODEL-DAY-006
#[test]
fn partial_observed_day_is_topped_up_from_forecast() {
    let mut observed_today = day(date(9, 20), 18.0, 17.0, 19.0, 90.0);
    observed_today.hours_covered = 6.0;
    observed_today.hours_rh90 = 6.0;
    let mut forecast_today = day(date(9, 20), 24.0, 20.0, 28.0, 60.0);
    forecast_today.hours_covered = 12.0;
    forecast_today.is_forecast = true;

    let series = build_series(vec![observed_today], vec![forecast_today]);
    assert_eq!(series.len(), 1);
    let merged = &series[0];
    assert_eq!(merged.hours_covered, 18.0);
    assert!((merged.temp_mean_c - 22.0).abs() < 1e-9);
    assert_eq!(merged.temp_min_c, 17.0);
    assert_eq!(merged.temp_max_c, 28.0);
    assert!((merged.rh_mean - 70.0).abs() < 1e-9);
    assert_eq!(merged.hours_rh90, 6.0);
    assert!(merged.is_forecast);
}

#[test]
fn complete_observed_day_ignores_forecast() {
    let observed = day(date(9, 19), 18.0, 17.0, 19.0, 90.0);
    let mut forecast = day(date(9, 19), 30.0, 25.0, 35.0, 40.0);
    forecast.is_forecast = true;
    let series = build_series(vec![observed.clone()], vec![forecast]);
    assert_eq!(series, vec![observed]);
}

// @spec DIS-MODEL-DAY-005
#[test]
fn each_forecast_point_is_three_hours_of_humidity_and_wetness() {
    // 15:00, 18:00 and 21:00 local on 9/20 (UTC-4).
    let points = [
        point(20, 19, 70.0, 90.0, 0.0),
        point(20, 22, 72.0, 89.9, 0.2),
        point(21, 1, 68.0, 70.0, 0.0),
    ];
    let days = forecast_days(&points, &eastern(), None);
    assert_eq!(days.len(), 1);
    let d = &days[0];
    assert_eq!(d.hours_covered, 9.0);
    assert_eq!(d.hours_rh90, 3.0, "RH of exactly 90% counts");
    assert_eq!(d.leaf_wetness_hours, 6.0, "humid or wet, not dry");
    assert!((d.precip_mm - 0.2).abs() < 1e-9);
}

// @spec DIS-MODEL-DAY-006
#[test]
fn observed_and_forecast_days_join_into_one_sorted_series() {
    let mut partial = day(date(9, 20), 20.0, 15.0, 25.0, 80.0);
    partial.hours_covered = 18.0;
    partial.precip_mm = 1.0;
    let sensor = crate::models::weather_basis::Basis {
        kind: crate::models::weather_basis::BasisKind::Sensor,
        source: "Patio".into(),
        distance_miles: None,
        approximate: false,
    };
    partial.basis = Some(sensor.clone());
    let mut rest_of_today = day(date(9, 20), 20.0, 14.0, 26.0, 80.0);
    rest_of_today.hours_covered = 9.0;
    rest_of_today.precip_mm = 2.0;
    rest_of_today.is_forecast = true;
    let mut tomorrow = day(date(9, 21), 22.0, 17.0, 27.0, 70.0);
    tomorrow.is_forecast = true;
    let full = day(date(9, 19), 18.0, 12.0, 24.0, 85.0);

    let series = build_series(
        vec![partial, full.clone()],
        vec![tomorrow.clone(), rest_of_today],
    );
    let dates: Vec<_> = series.iter().map(|d| d.date).collect();
    assert_eq!(dates, vec![date(9, 19), date(9, 20), date(9, 21)]);
    assert_eq!(series[0], full);
    assert_eq!(series[1].hours_covered, 24.0, "capped at 24");
    assert_eq!(series[1].precip_mm, 3.0);
    assert_eq!((series[1].temp_min_c, series[1].temp_max_c), (14.0, 26.0));
    assert_eq!(series[1].basis, Some(sensor), "the observed hours' basis");
    assert_eq!(series[2], tomorrow);
    assert_eq!(series[2].basis, None, "a forecast-only day has none");
}

// @spec DIS-MODEL-DAY-007
#[test]
fn hours_the_record_has_reported_are_not_counted_again() {
    // The record reported through 14:00 UTC; the 12:00 forecast point is already observed.
    let last_observed = Utc.with_ymd_and_hms(2026, 9, 21, 14, 0, 0).unwrap();
    let points = [
        point(21, 12, 70.0, 95.0, 0.0),
        point(21, 15, 72.0, 95.0, 0.0),
        point(21, 18, 74.0, 95.0, 0.0),
    ];
    let days = forecast_days(&points, &eastern(), Some(last_observed));
    let total: f64 = days.iter().map(|d| d.hours_covered).sum();
    assert_eq!(total, 6.0, "only the 15:00 and 18:00 points remain");
}
