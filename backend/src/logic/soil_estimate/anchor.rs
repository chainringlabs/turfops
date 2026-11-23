//! The estimate where it stands in for a measured series: shifted to meet the
//! measurements at the edges of a gap, so a change of basis is not a step that reads as
//! a threshold crossing.

#[cfg(test)]
#[path = "anchor_tests.rs"]
mod tests;

use super::model::{DailyAir, Model};
use chrono::{Duration, NaiveDate};
use std::collections::BTreeMap;

/// A soil-temperature series by day.
pub type Days = BTreeMap<NaiveDate, f64>;

/// Measured days this near a gap's edge set the offset there.
// @spec WX-EST-ANCHOR-001
pub const EDGE_DAYS: i64 = 7;
/// An open gap's offset is gone this many days after the last measured day.
// @spec WX-EST-ANCHOR-003
pub const FADE_DAYS: i64 = 30;

/// Measured minus estimate, averaged over the measured days among the `EDGE_DAYS` from
/// `edge` going `step` (−1 back, +1 forward). `None` when none of them has both.
// @spec WX-EST-ANCHOR-001
fn offset_at(edge: NaiveDate, step: i64, estimate: &Days, measured: &Days) -> Option<f64> {
    let differences: Vec<f64> = (0..EDGE_DAYS)
        .map(|n| edge + Duration::days(n * step))
        .filter_map(|day| Some(measured.get(&day)? - estimate.get(&day)?))
        .collect();
    (!differences.is_empty()).then(|| differences.iter().sum::<f64>() / differences.len() as f64)
}

/// The anchored value for each estimated day that has no measured value.
// @spec WX-EST-ANCHOR-002, WX-EST-ANCHOR-003, WX-EST-ANCHOR-004
pub fn anchored(estimate: &Days, measured: &Days) -> Days {
    estimate
        .iter()
        .filter(|(day, _)| !measured.contains_key(day))
        .map(|(day, value)| {
            let shifted = value + offset_on(*day, estimate, measured);
            (*day, (shifted * 10.0).round() / 10.0)
        })
        .collect()
}

/// The offset an unmeasured day's estimate is shifted by.
fn offset_on(day: NaiveDate, estimate: &Days, measured: &Days) -> f64 {
    let before = measured.range(..day).next_back().map(|(d, _)| *d);
    let after = measured.range(day..).next().map(|(d, _)| *d);
    let Some(before) = before else {
        // Before a source's first value there is nothing to meet.
        return 0.0;
    };
    let start = offset_at(before, -1, estimate, measured);
    let Some(after) = after else {
        let since = (day - before).num_days();
        let left = (1.0 - since as f64 / FADE_DAYS as f64).max(0.0);
        return start.unwrap_or(0.0) * left;
    };
    let end = offset_at(after, 1, estimate, measured);
    match (start, end) {
        (Some(start), Some(end)) => {
            let along = (day - before).num_days() as f64 / (after - before).num_days() as f64;
            start + (end - start) * along
        }
        (Some(only), None) | (None, Some(only)) => only,
        (None, None) => 0.0,
    }
}

/// The estimate for each day after `after` that the model can reach on `air`, a daily
/// air series that runs on into the forecast, meeting the measured series it follows.
// @spec WX-EST-FWD-001, WX-EST-FWD-002
pub fn forward(model: &Model, air: &DailyAir, measured: &Days, after: NaiveDate) -> Days {
    let estimate: Days = model.series(air).into_iter().collect();
    let mut ahead = anchored(&estimate, measured);
    ahead.retain(|day, _| *day > after);
    ahead
}
