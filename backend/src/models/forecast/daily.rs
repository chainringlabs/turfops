//! A forecast's points grouped into the lawn's local days.

use super::{DailyForecast, ForecastPoint, WeatherCondition};
use crate::clock::Clock;
use chrono::NaiveDate;
use std::collections::BTreeMap;

/// Daily summaries of `points`, oldest first. Days are the lawn's: UTC dates run 8 pm to
/// 8 pm Eastern, which puts an afternoon high and the next dawn low in different "days".
// @spec WX-SRC-OWM-001, WX-SRC-OWM-002
pub fn summarize(points: &[ForecastPoint], clock: &Clock) -> Vec<DailyForecast> {
    let mut by_date: BTreeMap<NaiveDate, Vec<&ForecastPoint>> = BTreeMap::new();
    for point in points {
        by_date
            .entry(clock.local_date(point.timestamp))
            .or_default()
            .push(point);
    }
    by_date
        .into_iter()
        .map(|(date, points)| day(date, &points))
        .collect()
}

fn day(date: NaiveDate, points: &[&ForecastPoint]) -> DailyForecast {
    let hours_covered: u32 = points.iter().map(|p| p.slot_hours).sum();
    let hours = f64::from(hours_covered.max(1));
    let over_hours = |value: fn(&ForecastPoint) -> f64| {
        points
            .iter()
            .map(|p| value(p) * f64::from(p.slot_hours))
            .sum::<f64>()
            / hours
    };
    let temps = || points.iter().map(|p| p.temp_f);
    DailyForecast {
        date,
        high_temp_f: temps().fold(f64::NEG_INFINITY, f64::max),
        low_temp_f: temps().fold(f64::INFINITY, f64::min),
        avg_humidity: over_hours(|p| p.humidity_percent),
        // A sum over part of a day would read as a dry day.
        total_precipitation_mm: points.iter().map(|p| p.precipitation_mm).sum(),
        max_precipitation_prob: points
            .iter()
            .filter_map(|p| p.precipitation_prob)
            .reduce(f64::max),
        dominant_condition: dominant(points),
        avg_wind_speed_mph: over_hours(|p| p.wind_speed_mph),
        max_wind_gust_mph: points
            .iter()
            .filter_map(|p| p.wind_gust_mph)
            .reduce(f64::max),
        hours_covered,
    }
}

/// The condition covering the most hours; the earliest of the day when two tie.
fn dominant(points: &[&ForecastPoint]) -> WeatherCondition {
    let mut hours: Vec<(WeatherCondition, u32)> = Vec::new();
    for point in points {
        match hours
            .iter_mut()
            .find(|(c, _)| *c == point.weather_condition)
        {
            Some((_, total)) => *total += point.slot_hours,
            None => hours.push((point.weather_condition, point.slot_hours)),
        }
    }
    let most = hours.iter().map(|(_, total)| *total).max().unwrap_or(0);
    hours
        .into_iter()
        .find(|(_, total)| *total == most)
        .map(|(condition, _)| condition)
        .unwrap_or_default()
}
