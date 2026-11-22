//! Resolving a value: for one input on one of the lawn's days, which source speaks.
//! The highest-ranked source that covers the day does, unless a source above it is still
//! expected to report that day.

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;

use crate::db::weather_queries::days::{HourlyDay, Stat};
use crate::models::weather_basis::{Basis, BasisSpan};
use crate::models::weather_record::{DailyRow, Input, Source};
use chrono::{Duration, NaiveDate};
use std::collections::BTreeMap;

/// An hourly source covers a day for an input with this many valid hours of it.
// @spec WX-REC-RES-002
pub const COVERING_HOURS: i64 = 12;

/// A day's air from one source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DayAir {
    pub max_f: f64,
    pub min_f: f64,
    pub mean_f: f64,
}

/// One source in an input's plan, as resolution sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate<T> {
    pub source: Source,
    /// The days this source covers, with its value for each.
    pub covered: BTreeMap<NaiveDate, T>,
    /// The lawn day of the source's newest value of the input, while that value is
    /// within the source's age limit: the source is still reporting and has got this
    /// far. `None` when it has no value or has gone quiet.
    pub reached: Option<NaiveDate>,
}

impl<T> Candidate<T> {
    /// Whether the source has not finished reporting `day`: it is still reporting, has
    /// not passed the day, and does not cover it yet.
    // @spec WX-REC-RES-010
    fn pending(&self, day: NaiveDate) -> bool {
        !self.covered.contains_key(&day) && self.reached.is_some_and(|reached| reached <= day)
    }
}

/// Whether a day a higher-ranked source has not reached is left empty or filled from
/// below. Rain is filled: its newest days come from the hourly analysis until the grid
/// publishes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    LeavesEmpty,
    FilledFromBelow,
}

/// The rule for an input.
// @spec WX-REC-RES-008, WX-REC-RES-010
pub fn pending_rule(input: Input) -> Pending {
    match input {
        Input::Rain => Pending::FilledFromBelow,
        _ => Pending::LeavesEmpty,
    }
}

/// A resolved value and the place in the plan it came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolved<T> {
    pub value: T,
    /// Index into the candidates.
    pub from: usize,
}

/// Each day from `from` through `through` that a source speaks for. `candidates` are in
/// plan order. A day with no entry has no value.
// @spec WX-REC-RES-001, WX-REC-FAIL-005
pub fn resolve<T: Clone>(
    candidates: &[Candidate<T>],
    from: NaiveDate,
    through: NaiveDate,
    rule: Pending,
) -> BTreeMap<NaiveDate, Resolved<T>> {
    let mut resolved = BTreeMap::new();
    let mut day = from;
    while day <= through {
        for (rank, candidate) in candidates.iter().enumerate() {
            if let Some(value) = candidate.covered.get(&day) {
                let value = value.clone();
                resolved.insert(day, Resolved { value, from: rank });
                break;
            }
            if rule == Pending::LeavesEmpty && candidate.pending(day) {
                break;
            }
        }
        day += Duration::days(1);
    }
    resolved
}

/// The basis of a resolved series: neighbouring days of the series on one source
/// collapsed into a span.
// @spec WX-REC-BASIS-003
pub fn spans<T>(
    input: Input,
    resolved: &BTreeMap<NaiveDate, Resolved<T>>,
    candidates: &[Candidate<T>],
) -> Vec<BasisSpan> {
    let mut spans: Vec<(usize, BasisSpan)> = Vec::new();
    for (day, value) in resolved {
        match spans.last_mut() {
            Some((rank, span)) if *rank == value.from => span.to = *day,
            _ => spans.push((
                value.from,
                BasisSpan {
                    input,
                    from: *day,
                    to: *day,
                    basis: Basis::from(&candidates[value.from].source),
                },
            )),
        }
    }
    spans.into_iter().map(|(_, span)| span).collect()
}

/// The days an hourly source covers for one of its values.
// @spec WX-REC-RES-002
pub fn covered_by_hours<T>(
    days: &[HourlyDay],
    value: impl Fn(&HourlyDay) -> (i64, Option<T>),
) -> BTreeMap<NaiveDate, T> {
    days.iter()
        .filter_map(|day| {
            let (hours, value) = value(day);
            (hours >= COVERING_HOURS).then_some((day.day, value?))
        })
        .collect()
}

/// The days a daily source covers for one of its values, each assigned to the lawn day
/// `shift_days` after the source's own.
// @spec WX-REC-RES-002, WX-REC-RES-007
pub fn covered_by_days<T>(
    rows: &[DailyRow],
    shift_days: i64,
    value: impl Fn(&DailyRow) -> Option<T>,
) -> BTreeMap<NaiveDate, T> {
    rows.iter()
        .filter_map(|row| Some((row.day + Duration::days(shift_days), value(row)?)))
        .collect()
}

/// A grid's precipitation for its day D fell mostly on the lawn's day D − 1: the grid's
/// day ends at 12:00 UTC.
// @spec WX-REC-RES-007
pub const GRID_RAIN_SHIFT_DAYS: i64 = -1;

/// A mean of an hourly source's day, as `covered_by_hours` wants it.
pub fn mean(stat: &Stat) -> (i64, Option<f64>) {
    (stat.hours, stat.mean)
}

/// A day's air from an hourly source: the extremes and the mean of its hours.
// @spec WX-REC-API-007
pub fn air_of_hours(day: &HourlyDay) -> (i64, Option<DayAir>) {
    let air = (|| {
        Some(DayAir {
            max_f: day.air_max?,
            min_f: day.air_min?,
            mean_f: day.air.mean?,
        })
    })();
    (day.air.hours, air)
}

/// A day's air from a grid: its maximum and minimum, and their midpoint as the mean.
// @spec WX-REC-API-007
pub fn air_of_grid(row: &DailyRow) -> Option<DayAir> {
    let (max_f, min_f) = (row.air_max_f?, row.air_min_f?);
    Some(DayAir {
        max_f,
        min_f,
        mean_f: (max_f + min_f) / 2.0,
    })
}
