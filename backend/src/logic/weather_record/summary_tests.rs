//! The soil trend.

use super::super::hourly::Hour;
use super::trend;
use crate::models::Trend;
use chrono::{Duration, TimeZone, Utc};

/// `count` hours ending at hour `last` of 2026-07-20 UTC, all at `value`.
fn block(last: i64, count: i64, value: f64, of_day: Option<f64>) -> Vec<Hour> {
    let base = Utc.with_ymd_and_hms(2026, 7, 18, 0, 0, 0).unwrap();
    (0..count)
        .map(|back| {
            let at = base + Duration::hours(last - back);
            Hour {
                at,
                day: at.date_naive(),
                value,
                rank: 0,
                of_day,
            }
        })
        .rev()
        .collect()
}

fn joined(parts: [Vec<Hour>; 2]) -> Vec<Hour> {
    parts.into_iter().flatten().collect()
}

// @spec WX-REC-API-004
#[test]
fn more_than_two_degrees_either_way_is_a_trend() {
    let hours = |before: f64, recent: f64| {
        joined([block(23, 24, before, None), block(47, 24, recent, None)])
    };
    assert_eq!(trend(&hours(60.0, 62.1)), Trend::Rising);
    assert_eq!(trend(&hours(60.0, 57.9)), Trend::Falling);
    assert_eq!(trend(&hours(60.0, 62.0)), Trend::Stable);
    assert_eq!(trend(&hours(60.0, 58.0)), Trend::Stable);
}

// @spec WX-REC-API-004
#[test]
fn without_hours_in_both_blocks_the_trend_is_unknown() {
    assert_eq!(trend(&[]), Trend::Unknown);
    assert_eq!(trend(&block(23, 24, 60.0, None)), Trend::Unknown);
    // The probe dropped most hours, but each block still has some.
    let sparse = joined([block(10, 3, 60.0, None), block(47, 4, 65.0, None)]);
    assert_eq!(trend(&sparse), Trend::Rising);
}

// @spec WX-REC-API-004
#[test]
fn on_a_daily_basis_the_newest_day_is_compared_with_the_day_before() {
    let days = joined([
        block(23, 24, 60.0, Some(60.0)),
        block(47, 24, 56.0, Some(56.0)),
    ]);
    assert_eq!(trend(&days), Trend::Falling);
    let one_day = block(47, 24, 56.0, Some(56.0));
    assert_eq!(trend(&one_day), Trend::Unknown);
    let gap = joined([
        block(23, 24, 60.0, Some(60.0)),
        block(71, 24, 56.0, Some(56.0)),
    ]);
    assert_eq!(trend(&gap), Trend::Unknown, "the day before has no value");
}

// @spec WX-REC-API-004
#[test]
fn a_change_of_source_between_the_two_periods_is_not_a_trend() {
    let mut hours = joined([block(23, 24, 60.0, None), block(47, 24, 70.0, None)]);
    for hour in &mut hours[..24] {
        hour.rank = 1;
    }
    assert_eq!(trend(&hours), Trend::Unknown);
}
