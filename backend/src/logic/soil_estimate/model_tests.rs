//! The regression: inputs, coverage, the fit and the series.

use super::model::*;
use chrono::{Datelike, Duration, NaiveDate};
use std::collections::BTreeMap;

fn d(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

/// Air on every day from `from` for `days` days, from `value(day index)`.
fn air(from: NaiveDate, days: i64, value: impl Fn(i64) -> f64) -> DailyAir {
    (0..days)
        .map(|i| (from + Duration::days(i), value(i)))
        .collect()
}

fn season(day: NaiveDate) -> (f64, f64) {
    let angle = std::f64::consts::TAU * f64::from(day.ordinal()) / 365.25;
    (angle.sin(), angle.cos())
}

// @spec WX-EST-MODEL-001
#[test]
fn inputs_are_the_five_trailing_means_then_sine_cosine_and_one() {
    // Air equal to the day index: day 29 is 29.0, the day before 28.0, and so on.
    let series = air(d(2024, 3, 1), 30, |i| i as f64);
    let day = d(2024, 3, 30);
    let got = features(&series, day).expect("thirty full days");
    let (sin, cos) = season(day);
    let want = [29.0, 28.0, 26.0, 22.5, 14.5, sin, cos, 1.0];
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 1e-9, "{got:?} vs {want:?}");
    }
}

// @spec WX-EST-MODEL-002
#[test]
fn a_window_with_under_80_percent_of_its_days_gives_no_inputs() {
    let day = d(2024, 3, 30);
    let mut series = air(d(2024, 3, 1), 30, |_| 50.0);
    // 30-day window: 24 of 30 is exactly 80 %, so six missing days still pass.
    for i in 0..6 {
        series.remove(&(d(2024, 3, 1) + Duration::days(i)));
    }
    assert!(features(&series, day).is_some());
    series.remove(&d(2024, 3, 7));
    assert!(features(&series, day).is_none(), "23 of 30 is under 80 %");
}

// @spec WX-EST-MODEL-002
#[test]
fn the_day_itself_must_have_air() {
    let day = d(2024, 3, 30);
    let mut series = air(d(2024, 3, 1), 30, |_| 50.0);
    series.remove(&day);
    assert!(
        features(&series, day).is_none(),
        "the 1-day window is empty"
    );
}

// @spec WX-EST-MODEL-002
#[test]
fn a_three_day_window_needs_all_three_days() {
    // Two of three is 67 %, under the 80 % the window needs.
    let day = d(2024, 3, 30);
    let mut series = air(d(2024, 3, 1), 30, |_| 50.0);
    series.remove(&d(2024, 3, 29));
    assert!(features(&series, day).is_none());
}

/// Day-to-day weather: a repeatable pseudo-random value in −8..8 °F. A sum of sinusoids
/// would make the trailing means linearly dependent and leave the fit without a solution.
fn weather(day: i64) -> f64 {
    let mixed = (day as u64)
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((mixed >> 33) % 1600) as f64 / 100.0 - 8.0
}

fn synthetic() -> (DailyAir, BTreeMap<NaiveDate, f64>, Features) {
    let truth = [0.10, 0.15, 0.20, 0.15, 0.10, -3.0, -6.0, 18.0];
    let start = d(2012, 1, 1);
    // Two years of air with a seasonal cycle and day-to-day variation.
    let series = air(start, 760, |i| {
        let t = i as f64;
        55.0 - 22.0 * (std::f64::consts::TAU * t / 365.25).cos() + weather(i)
    });
    let soil = series
        .keys()
        .filter_map(|&day| {
            let x = features(&series, day)?;
            Some((day, x.iter().zip(truth).map(|(a, b)| a * b).sum::<f64>()))
        })
        .collect();
    (series, soil, truth)
}

// @spec WX-EST-MODEL-001
#[test]
fn the_fit_recovers_the_coefficients_that_made_the_soil() {
    let (series, soil, truth) = synthetic();
    let rows = training_rows(&series, &soil, d(2012, 1, 1));
    let model = fit(&rows).expect("a unique solution");
    for (got, want) in model.coefficients.iter().zip(truth) {
        assert!((got - want).abs() < 1e-6, "{:?}", model.coefficients);
    }
}

// @spec WX-EST-MODEL-001
#[test]
fn too_few_rows_or_no_unique_solution_is_no_model() {
    let (series, soil, _) = synthetic();
    let rows = training_rows(&series, &soil, d(2012, 1, 1));
    assert!(fit(&rows[..TERMS - 1]).is_none(), "fewer rows than terms");
    let same = vec![rows[0]; 40];
    assert!(fit(&same).is_none(), "identical rows have no unique fit");
}

// @spec WX-EST-CAL-004
#[test]
fn training_rows_need_soil_and_usable_inputs_on_or_after_the_start() {
    let (series, mut soil, _) = synthetic();
    let from = d(2013, 1, 1);
    soil.remove(&d(2013, 6, 1));
    let rows = training_rows(&series, &soil, from);
    let expected = soil.keys().filter(|day| **day >= from).count();
    assert_eq!(rows.len(), expected);
    assert!(
        soil.contains_key(&d(2012, 6, 1)),
        "earlier days exist but are left out"
    );
}

// @spec WX-EST-MODEL-003
#[test]
fn an_estimate_is_rounded_to_a_tenth_of_a_degree() {
    let model = Model {
        coefficients: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 51.2345],
    };
    assert_eq!(
        model.estimate(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        51.2
    );
}

// @spec WX-EST-MODEL-003, WX-EST-FWD-001
#[test]
fn the_series_has_one_value_per_day_with_usable_inputs_in_date_order() {
    let model = Model {
        coefficients: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    };
    let series = air(d(2024, 3, 1), 40, |i| 40.0 + i as f64);
    let out = model.series(&series);
    // The first day whose 30-day window reaches 80 % is the 24th day.
    assert_eq!(out.first().map(|p| p.0), Some(d(2024, 3, 24)));
    assert_eq!(out.len(), 17);
    assert!(out.windows(2).all(|w| w[0].0 < w[1].0));
    assert_eq!(out.last(), Some(&(d(2024, 4, 9), 79.0)));
}
