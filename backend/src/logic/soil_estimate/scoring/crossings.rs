//! Every crossing a consuming component dates, and its date in one year of a daily soil
//! series, found with that component's own code.

use super::super::scores::Direction;
use crate::logic::seasonal_plan::crossings::{find_threshold_crossings, THRESHOLDS};
use crate::logic::timing::series::{self, Crossing, SoilPoint};
use crate::logic::timing::windows::{specs_for, Boundary};
use crate::models::lawn_profile::GrassType;
use crate::models::seasonal_plan::DailySoilTempAvg;
use chrono::{Duration, NaiveDate};
use std::collections::BTreeMap;

/// Days of soil before a scan starts that the timing windows' smoothing can reach.
const SMOOTHING_LEAD_DAYS: i64 = 10;

/// A crossing a consumer dates.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredCrossing {
    pub depth_cm: u32,
    pub threshold_f: f64,
    pub direction: Direction,
}

impl From<series::Direction> for Direction {
    fn from(direction: series::Direction) -> Self {
        match direction {
            series::Direction::Rising => Direction::Rising,
            series::Direction::Falling => Direction::Falling,
        }
    }
}

/// Every soil crossing in the timing windows of a cool-season lawn, in window order.
fn window_crossings() -> Vec<Crossing> {
    fn collect(boundary: &Boundary, out: &mut Vec<Crossing>) {
        match boundary {
            Boundary::Soil(crossing) => out.push(*crossing),
            Boundary::Earliest(options) => options.iter().for_each(|b| collect(b, out)),
            Boundary::BeforeFirstFreeze(_) | Boundary::Gdd(_) => {}
        }
    }
    let mut out = Vec::new();
    for spec in specs_for(GrassType::TallFescue) {
        let edges = [
            Some(&spec.opens),
            spec.ideal_from.as_ref(),
            spec.ideal_until.as_ref(),
        ];
        edges
            .into_iter()
            .flatten()
            .for_each(|b| collect(b, &mut out));
        collect(&spec.closes, &mut out);
    }
    out
}

fn scored(crossing: &Crossing) -> ScoredCrossing {
    ScoredCrossing {
        depth_cm: 5,
        threshold_f: crossing.threshold_f,
        direction: crossing.direction.into(),
    }
}

/// The 5 cm soil crossings of every timing window, without duplicates.
// @spec WX-EST-SCORE-001
pub fn timing_crossings() -> Vec<ScoredCrossing> {
    let mut out: Vec<ScoredCrossing> = Vec::new();
    for crossing in window_crossings().iter().map(scored) {
        if !out.contains(&crossing) {
            out.push(crossing);
        }
    }
    out
}

fn plan_direction(name: &str) -> Direction {
    if name.ends_with("_falling") {
        Direction::Falling
    } else {
        Direction::Rising
    }
}

/// The 10 cm thresholds of the seasonal plan (which include the outlook's).
// @spec WX-EST-SCORE-001
pub fn plan_crossings() -> Vec<ScoredCrossing> {
    THRESHOLDS
        .iter()
        .map(|&(name, threshold_f)| ScoredCrossing {
            depth_cm: 10,
            threshold_f,
            direction: plan_direction(name),
        })
        .collect()
}

/// The day of year on which `crossing` is dated in `year` of `soil`, by the rule of the
/// component that dates it: the timing windows' confirmed run at 5 cm, the seasonal
/// plan's held run at 10 cm. A date in the following January counts on from December 31.
/// `None` when that rule finds no datable crossing.
// @spec WX-EST-SCORE-001
pub fn date_in_year(
    crossing: &ScoredCrossing,
    soil: &BTreeMap<NaiveDate, f64>,
    year: i32,
) -> Option<u32> {
    let date = match crossing.depth_cm {
        5 => timing_date(crossing, soil, year),
        10 => plan_date(crossing, soil, year),
        _ => None,
    }?;
    let new_year = NaiveDate::from_ymd_opt(year, 1, 1)?;
    u32::try_from((date - new_year).num_days() + 1).ok()
}

/// The timing windows' historical date: a confirmed run that does not follow a data gap.
fn timing_date(
    crossing: &ScoredCrossing,
    soil: &BTreeMap<NaiveDate, f64>,
    year: i32,
) -> Option<NaiveDate> {
    let rule = window_crossings()
        .into_iter()
        .find(|candidate| scored(candidate) == *crossing)?;
    let from = rule.scan_from.in_season(year)? - Duration::days(SMOOTHING_LEAD_DAYS);
    let to = rule.scan_to.in_season(year)?;
    let daily: Vec<SoilPoint> = soil
        .range(from..=to)
        .map(|(&date, &temp_f)| SoilPoint {
            date,
            temp_f,
            is_forecast: false,
        })
        .collect();
    let found = series::detect(&series::smooth(&daily), year, &rule)?;
    (found.confirmed() && !found.after_gap).then_some(found.date)
}

/// The seasonal plan's crossing for the year, from that calendar year's daily rows.
fn plan_date(
    crossing: &ScoredCrossing,
    soil: &BTreeMap<NaiveDate, f64>,
    year: i32,
) -> Option<NaiveDate> {
    let first = NaiveDate::from_ymd_opt(year, 1, 1)?;
    let last = NaiveDate::from_ymd_opt(year, 12, 31)?;
    let rows: Vec<DailySoilTempAvg> = soil
        .range(first..=last)
        .map(|(&date, &avg_temp_f)| DailySoilTempAvg { date, avg_temp_f })
        .collect();
    let (name, _) = THRESHOLDS.iter().find(|&&(name, threshold_f)| {
        threshold_f == crossing.threshold_f && plan_direction(name) == crossing.direction
    })?;
    find_threshold_crossings(year, &rows)
        .into_iter()
        .find(|found| found.threshold_name == *name)
        .map(|found| found.crossing_date)
}
