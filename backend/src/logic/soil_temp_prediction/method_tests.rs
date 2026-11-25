//! Which method the 10 cm outlook uses, and what the estimated method answers.

use super::*;
use crate::logic::soil_estimate::model::Model;
use crate::models::weather_basis::{Basis, BasisKind};
use chrono::Duration;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 4, 20).unwrap()
}

fn day(offset: i64) -> NaiveDate {
    today() + Duration::days(offset)
}

fn station() -> Basis {
    Basis {
        kind: BasisKind::Station,
        source: "PA Avondale 2 N".into(),
        distance_miles: Some(2.1),
        approximate: false,
    }
}

fn estimate_basis() -> Basis {
    Basis {
        kind: BasisKind::Estimate,
        source: "Soil temperature estimate".into(),
        distance_miles: None,
        approximate: false,
    }
}

/// Air over the 60 days before today: 50°F with a small daily swing.
fn recorded_air() -> Vec<(NaiveDate, f64)> {
    (1..=60)
        .rev()
        .map(|back| (day(-back), 50.0 + ((back % 5) as f64)))
        .collect()
}

/// Forecast air from today: rising 2°F a day from 56°F, for `days` days.
fn forecast_air(days: i64) -> Vec<(NaiveDate, f64)> {
    (0..days).map(|n| (day(n), 56.0 + 2.0 * n as f64)).collect()
}

/// The last 30 days' pairs: soil = air + 2, measured or not.
fn pairs(measured: bool) -> Vec<(NaiveDate, f64, f64, bool)> {
    recorded_air()
        .into_iter()
        .filter(|(d, _)| *d >= day(-30))
        .map(|(d, air)| (d, air, air + 2.0, measured))
        .collect()
}

fn soil_days(measured: bool) -> Vec<(NaiveDate, f64, bool)> {
    pairs(measured)
        .into_iter()
        .map(|(d, _, soil, measured)| (d, soil, measured))
        .collect()
}

/// An estimate whose 10 cm soil is the day's air.
fn estimate() -> EstimateModel {
    EstimateModel {
        model: Model {
            coefficients: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        },
        info: EstimateInfo {
            stations: vec!["Station A".into(), "Station B".into()],
            training_days: 4200,
            daily_error_f: 1.8,
        },
    }
}

fn nothing_datable(_: f64, _: CrossingDirection) -> bool {
    false
}

fn outlook(
    pairs: &[(NaiveDate, f64, f64, bool)],
    soil: &[(NaiveDate, f64, bool)],
    current: Option<(f64, Basis)>,
    estimate: Option<&EstimateModel>,
    forecast_days: i64,
    datable: &dyn Fn(f64, CrossingDirection) -> bool,
) -> Option<SoilTempForecast> {
    let recorded = recorded_air();
    let forecast = forecast_air(forecast_days);
    build_outlook(
        &OutlookInputs {
            pairs,
            recorded_air: &recorded,
            forecast_air: &forecast,
            soil_days: soil,
            current,
            estimate,
            datable,
        },
        today(),
    )
}

// @spec WX-OUTLOOK-METHOD-001, WX-OUTLOOK-MODEL-004
#[test]
fn measured_soil_with_enough_measured_days_is_fitted() {
    let estimate = estimate();
    let f = outlook(
        &pairs(true),
        &soil_days(true),
        Some((54.0, station())),
        Some(&estimate),
        10,
        &nothing_datable,
    )
    .expect("an outlook");
    assert_eq!(f.method, OutlookMethod::Fitted);
    assert_eq!(f.basis, Some(station()));
    assert!(f.model_info.is_some() && f.estimate_info.is_none());
}

// @spec WX-OUTLOOK-METHOD-002
#[test]
fn estimated_soil_or_too_few_measured_days_uses_the_estimate() {
    let estimate = estimate();
    // The current reading rests on the estimate.
    let f = outlook(
        &pairs(true),
        &soil_days(true),
        Some((54.0, estimate_basis())),
        Some(&estimate),
        10,
        &nothing_datable,
    )
    .unwrap();
    assert_eq!(f.method, OutlookMethod::Estimated);

    // Measured now, but 13 of the last 30 days measured.
    let mut thin = pairs(false);
    for pair in thin.iter_mut().rev().take(13) {
        pair.3 = true;
    }
    let f = outlook(
        &thin,
        &soil_days(false),
        Some((54.0, station())),
        Some(&estimate),
        10,
        &nothing_datable,
    )
    .unwrap();
    assert_eq!(f.method, OutlookMethod::Estimated);
    assert_eq!(f.basis, Some(station()), "the soil it starts from");
    assert!(f.model_info.is_none());
    assert_eq!(f.estimate_info, Some(estimate.info.clone()));
}

// @spec WX-OUTLOOK-MODEL-006, WX-OUTLOOK-METHOD-004, WX-OUTLOOK-MODEL-005
#[test]
fn the_estimated_method_is_medium_three_days_out_and_stops_at_the_tenth_forecast_day() {
    let estimate = estimate();
    let f = outlook(
        &pairs(false),
        &soil_days(false),
        Some((54.0, estimate_basis())),
        Some(&estimate),
        10,
        &nothing_datable,
    )
    .unwrap();
    let last = f.predictions.last().unwrap();
    assert_eq!(last.date, day(9), "no day past the tenth forecast day");
    for p in &f.predictions {
        let out = (p.date - today()).num_days();
        let expected = if out <= 3 {
            PredictionConfidence::Medium
        } else {
            PredictionConfidence::Low
        };
        assert_eq!(p.confidence, expected, "{} days out", out);
    }
    // The estimate is the day's air here: 56°F today, rising 2°F a day.
    let tomorrow = f.predictions.iter().find(|p| p.date == day(1)).unwrap();
    assert_eq!(tomorrow.predicted_soil_temp_f, 58.0);
}

// @spec WX-OUTLOOK-METHOD-003
#[test]
fn the_estimated_method_announces_only_crossings_the_estimate_may_date() {
    let estimate = estimate();
    let grub_only = |threshold: f64, direction: CrossingDirection| {
        threshold == 60.0 && direction == CrossingDirection::Rising
    };
    let f = outlook(
        &pairs(false),
        &soil_days(false),
        Some((54.0, estimate_basis())),
        Some(&estimate),
        10,
        &grub_only,
    )
    .unwrap();
    let names: Vec<&str> = f
        .threshold_crossings
        .iter()
        .map(|c| c.threshold_name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["Grub Control Window"],
        "65°F is crossed but not datable"
    );
}

// @spec WX-OUTLOOK-METHOD-002, WX-OUTLOOK-API-003
#[test]
fn with_no_fit_and_no_estimate_there_is_no_outlook() {
    assert!(outlook(
        &pairs(false),
        &soil_days(false),
        Some((54.0, estimate_basis())),
        None,
        10,
        &nothing_datable,
    )
    .is_none());
}
