use super::super::scores::Direction;
use super::crossings::*;
use crate::logic::seasonal_plan::crossings::THRESHOLDS;
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;

fn d(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

/// A year of soil that is `before` until `change` and `after` from it on.
fn step(year: i32, change: NaiveDate, before: f64, after: f64) -> BTreeMap<NaiveDate, f64> {
    d(year, 1, 1)
        .iter_days()
        .take_while(|day| day.year() == year)
        .map(|day| (day, if day < change { before } else { after }))
        .collect()
}

fn five(threshold_f: f64, direction: Direction) -> ScoredCrossing {
    ScoredCrossing {
        depth_cm: 5,
        threshold_f,
        direction,
    }
}

// @spec WX-EST-SCORE-001
#[test]
fn the_5_cm_list_is_every_soil_boundary_of_the_timing_windows() {
    let got = timing_crossings();
    for (threshold, direction) in [
        (45.0, Direction::Rising),
        (50.0, Direction::Rising),
        (55.0, Direction::Rising),
        (65.0, Direction::Rising),
        (75.0, Direction::Falling),
        (70.0, Direction::Falling),
        (65.0, Direction::Falling),
        (55.0, Direction::Falling),
        (40.0, Direction::Falling),
    ] {
        assert!(
            got.contains(&five(threshold, direction)),
            "{threshold} {direction:?}"
        );
    }
    assert_eq!(got.len(), 9, "no duplicates, nothing else: {got:?}");
}

// @spec WX-EST-SCORE-001
#[test]
fn the_10_cm_list_is_the_seasonal_plans_thresholds() {
    let got = plan_crossings();
    assert_eq!(got.len(), THRESHOLDS.len());
    assert!(got.iter().all(|c| c.depth_cm == 10));
    assert!(got.contains(&ScoredCrossing {
        depth_cm: 10,
        threshold_f: 45.0,
        direction: Direction::Falling
    }));
    assert!(got.contains(&ScoredCrossing {
        depth_cm: 10,
        threshold_f: 75.0,
        direction: Direction::Rising
    }));
}

// @spec WX-EST-SCORE-001
#[test]
fn a_5_cm_crossing_is_dated_by_the_timing_windows_rule() {
    // 40 °F until Apr 10, 60 °F after: the trailing 5-day mean reaches 55 °F on Apr 13
    // (40, 60, 60, 60, 60 → 56) and holds.
    let soil = step(2020, d(2020, 4, 10), 40.0, 60.0);
    let got = date_in_year(&five(55.0, Direction::Rising), &soil, 2020);
    assert_eq!(got, Some(d(2020, 4, 13).ordinal()));
}

// @spec WX-EST-SCORE-001
#[test]
fn a_10_cm_crossing_is_dated_by_the_seasonal_plans_rule() {
    // The 7-row trailing mean reaches 55 °F on Apr 15 (two days at 40, five at 60 → 54.3;
    // one at 40, six at 60 → 57.1).
    let soil = step(2020, d(2020, 4, 10), 40.0, 60.0);
    let crossing = ScoredCrossing {
        depth_cm: 10,
        threshold_f: 55.0,
        direction: Direction::Rising,
    };
    assert_eq!(
        date_in_year(&crossing, &soil, 2020),
        Some(d(2020, 4, 15).ordinal())
    );
}

// @spec WX-EST-SCORE-001
#[test]
fn a_year_in_which_the_threshold_is_never_crossed_has_no_date() {
    let soil = step(2020, d(2020, 4, 10), 40.0, 50.0);
    assert_eq!(
        date_in_year(&five(55.0, Direction::Rising), &soil, 2020),
        None
    );
}

// @spec WX-EST-SCORE-001
#[test]
fn a_5_cm_run_that_starts_right_after_a_hole_is_not_dated() {
    // The timing windows drop a crossing whose run begins after a data gap.
    let mut soil = step(2020, d(2020, 4, 10), 40.0, 60.0);
    for day in d(2020, 4, 1).iter_days().take(20) {
        soil.remove(&day);
    }
    assert_eq!(
        date_in_year(&five(55.0, Direction::Rising), &soil, 2020),
        None
    );
}
