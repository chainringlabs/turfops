//! The data file and `datable`.

use super::scores::*;
use super::scoring::crossings::{plan_crossings, timing_crossings};

fn file(crossings: &str) -> ScoreFile {
    ScoreFile::parse(&format!(
        r#"{{"generated_on":"2026-11-22","stations":113,"first_year":2012,"last_year":2025,
            "daily_error":[{{"depth_cm":5,"calibration":"NearestFive","rmse_f":3.5}}],
            "crossings":[{crossings}]}}"#
    ))
    .expect("valid score file")
}

fn row(threshold: f64, calibration: &str, median: f64, spread: f64) -> String {
    format!(
        r#"{{"depth_cm":5,"threshold_f":{threshold},"direction":"Rising","calibration":"{calibration}",
            "median_days":{median},"p80_days":13.0,"bias_days":-1.6,
            "spread_median_days":{spread},"spread_p80_days":13.0,"samples":1399}}"#
    )
}

// @spec WX-EST-DATE-001, WX-EST-DATE-002
#[test]
fn a_scored_crossing_that_beats_the_spread_between_years_is_datable_with_its_error() {
    let scores = file(&row(55.0, "NearestFive", 2.0, 6.0));
    let got = scores.datable(5, 55.0, Direction::Rising, Calibration::NearestFive);
    assert_eq!(
        got,
        Some(Accuracy {
            median_days: 2.0,
            p80_days: 13.0
        })
    );
}

// @spec WX-EST-DATE-001
#[test]
fn a_median_error_equal_to_or_above_the_spread_is_not_datable() {
    let equal = file(&row(40.0, "NearestFive", 4.0, 4.0));
    assert_eq!(
        equal.datable(5, 40.0, Direction::Rising, Calibration::NearestFive),
        None
    );
    let worse = file(&row(75.0, "NearestFive", 8.0, 6.0));
    assert_eq!(
        worse.datable(5, 75.0, Direction::Rising, Calibration::NearestFive),
        None
    );
}

// @spec WX-EST-DATE-001
#[test]
fn datable_is_judged_under_the_calibration_asked_for() {
    let rows = [
        row(40.0, "NearestFive", 4.0, 4.0),
        row(40.0, "StationAlone", 3.0, 4.0),
    ]
    .join(",");
    let scores = file(&rows);
    assert!(scores
        .datable(5, 40.0, Direction::Rising, Calibration::NearestFive)
        .is_none());
    assert!(scores
        .datable(5, 40.0, Direction::Rising, Calibration::StationAlone)
        .is_some());
}

// @spec WX-EST-DATE-003
#[test]
fn a_crossing_the_file_does_not_hold_is_not_datable() {
    let scores = file(&row(55.0, "NearestFive", 2.0, 6.0));
    for (depth, threshold, direction) in [
        (10, 55.0, Direction::Rising),
        (5, 56.0, Direction::Rising),
        (5, 55.0, Direction::Falling),
    ] {
        assert!(scores
            .datable(depth, threshold, direction, Calibration::NearestFive)
            .is_none());
    }
}

// @spec WX-EST-SCORE-003
#[test]
fn the_embedded_file_scores_every_crossing_a_consumer_dates_under_both_calibrations() {
    let scores = embedded();
    assert!(scores.stations >= 100, "scored on the whole network");
    for crossing in timing_crossings().into_iter().chain(plan_crossings()) {
        for calibration in [Calibration::NearestFive, Calibration::StationAlone] {
            assert!(
                scores
                    .crossings
                    .iter()
                    .any(|s| s.depth_cm == crossing.depth_cm
                        && s.threshold_f == crossing.threshold_f
                        && s.direction == crossing.direction
                        && s.calibration == calibration),
                "no score for {crossing:?} under {calibration:?}"
            );
        }
    }
}

// @spec WX-EST-SCORE-003
#[test]
fn the_embedded_file_gives_the_daily_error_at_both_depths_and_calibrations() {
    let scores = embedded();
    for depth in [5, 10] {
        for calibration in [Calibration::NearestFive, Calibration::StationAlone] {
            assert!(scores
                .daily_error
                .iter()
                .any(|e| e.depth_cm == depth && e.calibration == calibration && e.rmse_f > 0.0));
        }
    }
}
