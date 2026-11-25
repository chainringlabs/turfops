//! Builds the daily weather series the disease models run on: observed lake days,
//! with today's remainder and the coming days filled in from the forecast.

use crate::clock::Clock;
use crate::models::{DailyWeather, ForecastPoint, AFTERNOON_HOURS, DAWN_HOURS};
use chrono::{NaiveDate, Timelike};
use std::collections::BTreeMap;

/// RH (%) at or above which a leaf is assumed wet.
const LEAF_WETNESS_RH: f64 = 90.0;

/// Magnus-formula dew point (°C) from air temp (°C) and RH (%).
pub fn dew_point_c(temp_c: f64, rh: f64) -> f64 {
    let gamma = (rh.max(1.0) / 100.0).ln() + 17.625 * temp_c / (243.04 + temp_c);
    243.04 * gamma / (17.625 - gamma)
}

fn fahrenheit_to_celsius(f: f64) -> f64 {
    (f - 32.0) * 5.0 / 9.0
}

/// Aggregate forecast points into the lawn's local calendar days, each point on the local
/// date (and hour) of its own time and counted as its slot's length.
// @spec DIS-MODEL-DAY-003
/// Points at or before `after` (the newest lake observation) are dropped: those hours
/// are already observed.
pub fn forecast_days(
    points: &[ForecastPoint],
    clock: &Clock,
    after: Option<chrono::DateTime<chrono::Utc>>,
) -> Vec<DailyWeather> {
    let mut by_day: BTreeMap<NaiveDate, Vec<&ForecastPoint>> = BTreeMap::new();
    // @spec DIS-MODEL-DAY-007
    for point in points
        .iter()
        .filter(|p| after.is_none_or(|t| p.timestamp > t))
    {
        by_day
            .entry(clock.local_date(point.timestamp))
            .or_default()
            .push(point);
    }

    by_day
        .into_iter()
        .map(|(date, pts)| forecast_day(date, &pts, clock))
        .collect()
}

/// One local day of forecast points, each counted as its slot's length.
// @spec DIS-MODEL-DAY-005
fn forecast_day(date: NaiveDate, pts: &[&ForecastPoint], clock: &Clock) -> DailyWeather {
    let local_hour = |p: &&ForecastPoint| clock.local(p.timestamp).hour();
    let slot = |p: &ForecastPoint| f64::from(p.slot_hours);
    let hours: f64 = pts.iter().map(|p| slot(p)).sum();
    let temps: Vec<f64> = pts
        .iter()
        .map(|p| fahrenheit_to_celsius(p.temp_f))
        .collect();
    let humid = |p: &ForecastPoint| p.humidity_percent >= LEAF_WETNESS_RH;
    let raining = |p: &ForecastPoint| p.precipitation_mm.is_some_and(|mm| mm > 0.0);
    let hours_where = |wanted: &dyn Fn(&ForecastPoint) -> bool| -> f64 {
        pts.iter().filter(|p| wanted(p)).map(|p| slot(p)).sum()
    };
    DailyWeather {
        date,
        temp_mean_c: pts
            .iter()
            .zip(&temps)
            .map(|(p, t)| t * slot(p))
            .sum::<f64>()
            / hours,
        temp_min_c: temps.iter().copied().fold(f64::INFINITY, f64::min),
        temp_max_c: temps.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        rh_mean: pts
            .iter()
            .map(|p| p.humidity_percent * slot(p))
            .sum::<f64>()
            / hours,
        hours_rh90: hours_where(&humid),
        leaf_wetness_hours: hours_where(&|p| humid(p) || raining(p)),
        precip_mm: pts.iter().filter_map(|p| p.precipitation_mm).sum(),
        dew_point_max_c: pts
            .iter()
            .zip(&temps)
            .map(|(p, t)| dew_point_c(*t, p.humidity_percent))
            .fold(f64::NEG_INFINITY, f64::max),
        hours_covered: hours,
        is_forecast: true,
        covers_dawn: pts.iter().any(|p| DAWN_HOURS.contains(&local_hour(p))),
        covers_afternoon: pts.iter().any(|p| AFTERNOON_HOURS.contains(&local_hour(p))),
        basis: None,
    }
}

/// Combine two partial views of the same day, weighting means by hours covered.
fn merge_day(a: &DailyWeather, b: &DailyWeather) -> DailyWeather {
    let total = a.hours_covered + b.hours_covered;
    let weighted = |x: f64, y: f64| (x * a.hours_covered + y * b.hours_covered) / total;
    DailyWeather {
        date: a.date,
        temp_mean_c: weighted(a.temp_mean_c, b.temp_mean_c),
        temp_min_c: a.temp_min_c.min(b.temp_min_c),
        temp_max_c: a.temp_max_c.max(b.temp_max_c),
        rh_mean: weighted(a.rh_mean, b.rh_mean),
        hours_rh90: a.hours_rh90 + b.hours_rh90,
        leaf_wetness_hours: a.leaf_wetness_hours + b.leaf_wetness_hours,
        precip_mm: a.precip_mm + b.precip_mm,
        dew_point_max_c: a.dew_point_max_c.max(b.dew_point_max_c),
        hours_covered: total.min(24.0),
        is_forecast: a.is_forecast || b.is_forecast,
        covers_dawn: a.covers_dawn || b.covers_dawn,
        covers_afternoon: a.covers_afternoon || b.covers_afternoon,
        basis: a.basis.clone().or_else(|| b.basis.clone()),
    }
}

// @spec DIS-MODEL-DAY-006
/// Merge observed and forecast days into one ascending series. A fully observed day
/// wins outright; a partly observed day (the lake lags, so usually today) is topped up
/// with whatever forecast points remain for it.
pub fn build_series(observed: Vec<DailyWeather>, forecast: Vec<DailyWeather>) -> Vec<DailyWeather> {
    let mut by_day: BTreeMap<NaiveDate, DailyWeather> =
        observed.into_iter().map(|d| (d.date, d)).collect();
    for day in forecast {
        let merged = match by_day.get(&day.date) {
            Some(obs) if obs.hours_covered >= 24.0 => continue,
            Some(obs) => merge_day(obs, &day),
            None => day,
        };
        by_day.insert(merged.date, merged);
    }
    by_day.into_values().collect()
}

#[cfg(test)]
mod tests;
