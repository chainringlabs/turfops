//! 10 cm soil-temperature threshold crossings: finding each year's crossings from the
//! weather record's daily means, and summarizing them across years for a plan year.

use crate::logic::weather_record::daily::period_basis;
use crate::models::environmental::PeriodBasis;
use crate::models::seasonal_plan::*;
use crate::models::weather_basis::{BasisKind, Series};
use crate::models::weather_record::Input;
use chrono::{Datelike, NaiveDate};

/// Soil temperature thresholds (°F, 7-day rolling average) that trigger activities.
/// Each threshold has a name used as cache key and the temp value.
pub const THRESHOLDS: &[(&str, f64)] = &[
    ("soil_50f_rising", 50.0),  // Pre-emergent, aeration, overseeding low
    ("soil_55f_rising", 55.0),  // Spring N, broadleaf herbicide, overseeding peak
    ("soil_60f_rising", 60.0),  // Grub control start
    ("soil_65f_rising", 65.0),  // Grub/aeration/overseeding upper limit
    ("soil_75f_rising", 75.0),  // Grub upper limit
    ("soil_65f_falling", 65.0), // Fall activities begin
    ("soil_55f_falling", 55.0), // Fall herbicide lower
    ("soil_50f_falling", 50.0), // Overseeding/aeration lower on fall side
    ("soil_45f_falling", 45.0), // Winterizer window
];

/// The 10 cm crossings of one year: per threshold, the first run of 7-row trailing means
/// past it that holds 5 consecutive days, does not begin the year's data, and starts in
/// the threshold's half of the year (rising Jan–Jun, falling Jul–Dec). It is dated on the
/// run's first day with that day's mean. A run that starts the day after a missing date
/// dates the sensor coming back, not the soil, so that threshold has no crossing that year.
// @spec PLAN-CROSS-002, PLAN-CROSS-003, PLAN-CROSS-004, PLAN-CROSS-005, PLAN-CROSS-008, PLAN-CROSS-009
pub fn find_threshold_crossings(
    year: i32,
    daily_temps: &[DailySoilTempAvg],
) -> Vec<ThresholdCrossing> {
    let trailing: Vec<(NaiveDate, f64)> = daily_temps
        .windows(7)
        .map(|w| (w[6].date, w.iter().map(|d| d.avg_temp_f).sum::<f64>() / 7.0))
        .collect();

    THRESHOLDS
        .iter()
        .filter_map(|&(name, threshold)| {
            let falling = name.ends_with("_falling");
            let past = |mean: f64| {
                if falling {
                    mean <= threshold
                } else {
                    mean >= threshold
                }
            };
            let in_season = |date: NaiveDate| (date.month() >= 7) == falling;
            let start = first_held_run(&trailing, past, in_season)?;
            let (date, mean) = trailing[start];
            Some(ThresholdCrossing {
                year,
                threshold_name: name.to_string(),
                crossing_date: date,
                avg_soil_temp_f: mean,
                estimated: false,
            })
        })
        .collect()
}

/// Days a run must hold before it counts as a crossing.
const HOLD_DAYS: usize = 5;

/// Index of the first in-season held run's first day, or `None` (also when that run
/// follows a missing date).
fn first_held_run(
    trailing: &[(NaiveDate, f64)],
    past: impl Fn(f64) -> bool,
    in_season: impl Fn(NaiveDate) -> bool,
) -> Option<usize> {
    let follows = |i: usize| trailing[i - 1].0.succ_opt() == Some(trailing[i].0);
    let mut i = 0;
    while i < trailing.len() {
        if !past(trailing[i].1) {
            i += 1;
            continue;
        }
        let start = i;
        while i + 1 < trailing.len() && past(trailing[i + 1].1) && follows(i + 1) {
            i += 1;
        }
        let held = i + 1 - start >= HOLD_DAYS;
        if held && start > 0 && in_season(trailing[start].0) {
            return follows(start).then_some(start);
        }
        i += 1;
    }
    None
}

/// One threshold's crossings mapped onto `target_year` by month and day (Feb 29 → Feb 28):
/// the median (index n/2), earliest, latest and sample count.
// @spec PLAN-CROSS-006
pub fn aggregate_crossings(
    crossings: &[ThresholdCrossing],
    threshold_name: &str,
    target_year: i32,
) -> Option<AggregatedWindow> {
    let mut dates: Vec<NaiveDate> = crossings
        .iter()
        .filter(|c| c.threshold_name == threshold_name)
        .map(|c| {
            let d = c.crossing_date;
            NaiveDate::from_ymd_opt(target_year, d.month(), d.day())
                .or_else(|| NaiveDate::from_ymd_opt(target_year, 2, 28))
                .unwrap_or(d)
        })
        .collect();

    if dates.is_empty() {
        return None;
    }

    dates.sort();

    let earliest = *dates.first().unwrap();
    let latest = *dates.last().unwrap();
    let median = dates[dates.len() / 2];

    Some(AggregatedWindow {
        median,
        earliest,
        latest,
        sample_count: dates.len(),
    })
}

#[derive(Debug, Clone)]
pub struct AggregatedWindow {
    pub median: NaiveDate,
    pub earliest: NaiveDate,
    pub latest: NaiveDate,
    pub sample_count: usize,
}

/// The cached years whose crossings vote for `plan_year`: the at most 10 most recent
/// complete (settled) years before it.
pub fn voting_years(settled_years: &[i32], plan_year: i32) -> Vec<i32> {
    let mut years: Vec<i32> = settled_years
        .iter()
        .copied()
        .filter(|y| *y < plan_year)
        .collect();
    years.sort_unstable_by(|a, b| b.cmp(a));
    years.dedup();
    years.truncate(10);
    years.reverse();
    years
}

/// The crossings of every year one record series covers, and which of those years are
/// settled.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RecordCrossings {
    pub crossings: Vec<ThresholdCrossing>,
    pub settled_years: Vec<i32>,
    /// The 10 cm soil the crossings were found in.
    pub basis: Option<PeriodBasis>,
}

/// A threshold's value and whether it is crossed falling.
pub fn threshold(name: &str) -> Option<(f64, bool)> {
    THRESHOLDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|&(n, value)| (value, n.ends_with("_falling")))
}

/// Each year's crossings from one series of the record's daily 10 cm means: a year with
/// fewer than 30 days has none; a crossing any of whose five held days rests on the
/// estimate is marked estimated and kept only when `datable` says the estimate may date
/// that threshold and direction. A year is settled once it is over on the lawn's
/// calendar (`today`) and the series holds December 31 of it or later.
// @spec PLAN-CROSS-001, PLAN-CROSS-011, PLAN-CROSS-013
pub fn crossings_from_record(
    series: &Series<DailySoilTempAvg>,
    today: NaiveDate,
    datable: &dyn Fn(f64, bool) -> bool,
) -> RecordCrossings {
    let estimated_days: Vec<(NaiveDate, NaiveDate)> = series
        .basis
        .iter()
        .filter(|span| span.input == Input::SoilTemp10 && span.basis.kind == BasisKind::Estimate)
        .map(|span| (span.from, span.to))
        .collect();
    let estimated = |day: NaiveDate| {
        estimated_days
            .iter()
            .any(|(from, to)| *from <= day && day <= *to)
    };
    let newest = series.rows.last().map(|row| row.date);
    let mut years: Vec<i32> = series.rows.iter().map(|row| row.date.year()).collect();
    years.dedup();

    let mut crossings = Vec::new();
    let mut settled_years = Vec::new();
    for year in years {
        let days: Vec<DailySoilTempAvg> = series
            .rows
            .iter()
            .filter(|row| row.date.year() == year)
            .cloned()
            .collect();
        if days.len() >= 30 {
            for mut crossing in find_threshold_crossings(year, &days) {
                let held = (0..HOLD_DAYS as i64)
                    .map(|n| crossing.crossing_date + chrono::Duration::days(n));
                crossing.estimated = held.into_iter().any(estimated);
                let keep = !crossing.estimated
                    || threshold(&crossing.threshold_name)
                        .is_some_and(|(value, falling)| datable(value, falling));
                if keep {
                    crossings.push(crossing);
                }
            }
        }
        let year_end = NaiveDate::from_ymd_opt(year, 12, 31);
        let over = today.year() > year;
        if over
            && newest
                .zip(year_end)
                .is_some_and(|(newest, end)| newest >= end)
        {
            settled_years.push(year);
        }
    }
    let soil: Vec<_> = series
        .basis
        .iter()
        .filter(|span| span.input == Input::SoilTemp10)
        .cloned()
        .collect();
    RecordCrossings {
        crossings,
        settled_years,
        basis: period_basis(&soil),
    }
}
