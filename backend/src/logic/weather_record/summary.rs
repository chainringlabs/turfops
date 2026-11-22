//! The record's summary: the live reading, the last 168 hours in four figures, and the
//! soil trend.

use super::hourly::{hour_of, Hour, Hours};
use super::live::LiveReading;
use super::record::{Asked, WeatherRecord};
use crate::error::Result;
use crate::models::weather_basis::{Basis, Figure};
use crate::models::weather_record::Input;
use crate::models::Trend;
use chrono::Duration;

#[cfg(test)]
#[path = "summary_tests.rs"]
mod tests;

/// The hours a summary covers.
const WINDOW_HOURS: i64 = 168;
/// A change in the 10 cm soil mean of more than this, in °F, is a trend.
const TREND_STEP_F: f64 = 2.0;

/// Present conditions as the record knows them. Every figure carries its basis.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordSummary {
    pub live: LiveReading,
    pub soil_temp_7day_avg_f: Option<Figure<f64>>,
    pub ambient_temp_7day_avg_f: Option<Figure<f64>>,
    pub humidity_7day_avg: Option<Figure<f64>>,
    pub precipitation_7day_total_mm: Option<Figure<f64>>,
    pub soil_temp_trend: Trend,
}

impl WeatherRecord {
    /// The live reading and the 168 hours ending at the caller's now.
    // @spec WX-REC-API-003, WX-REC-API-004, WX-REC-FAIL-003
    pub async fn fetch_summary(&self, asked: Asked) -> Result<RecordSummary> {
        let view = self.view(asked).await?;
        let last = hour_of(asked.at.now);
        let first = last - Duration::hours(WINDOW_HOURS - 1);
        let soil = view.hours(Input::SoilTemp10, first, last).await?;
        let air = view.hours(Input::AirTemp, first, last).await?;
        let humidity = view.hours(Input::Humidity, first, last).await?;
        let rain = view.hours(Input::Rain, first, last).await?;
        Ok(RecordSummary {
            live: view.live_reading().await?,
            soil_temp_trend: trend(&soil.hours),
            soil_temp_7day_avg_f: figure(&soil, mean),
            ambient_temp_7day_avg_f: figure(&air, mean),
            humidity_7day_avg: figure(&humidity, mean),
            precipitation_7day_total_mm: figure(&rain, |values| values.iter().sum()),
        })
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// One figure over the hours that have a value, with the basis of the newest of them.
// @spec WX-REC-BASIS-004, WX-REC-FAIL-005
fn figure(hours: &Hours, of: fn(&[f64]) -> f64) -> Option<Figure<f64>> {
    let newest = hours.hours.last()?;
    let values: Vec<f64> = hours.hours.iter().map(|hour| hour.value).collect();
    Some(Figure {
        value: of(&values),
        basis: Basis::from(&hours.sources[newest.rank]),
        mixed: hours.hours.iter().any(|hour| hour.rank != newest.rank),
    })
}

/// Whether 10 cm soil is warming or cooling: the newest 24 hours against the 24 before,
/// or the newest day against the day before where the newest value is a daily one.
// @spec WX-REC-API-004
pub(super) fn trend(hours: &[Hour]) -> Trend {
    let Some(newest) = hours.last() else {
        return Trend::Unknown;
    };
    // The mean of the hours that pass, when they all rest on the newest hour's source.
    // A change of source between the two periods is a seam, not a trend.
    let mean_where = |wanted: &dyn Fn(&Hour) -> bool| {
        let block: Vec<&Hour> = hours.iter().filter(|h| wanted(h)).collect();
        let one_source = block.iter().all(|h| h.rank == newest.rank);
        let values: Vec<f64> = block.iter().map(|h| h.value).collect();
        (one_source && !values.is_empty()).then(|| mean(&values))
    };
    let (recent, before) = if newest.of_day.is_some() {
        let day_before = newest.day - Duration::days(1);
        (
            mean_where(&|h| h.day == newest.day),
            mean_where(&|h| h.day == day_before),
        )
    } else {
        let block = |from: i64, to: i64| {
            let start = newest.at - Duration::hours(from);
            let end = newest.at - Duration::hours(to);
            mean_where(&|h| h.at > start && h.at <= end)
        };
        (block(24, 0), block(48, 24))
    };
    match recent.zip(before).map(|(recent, before)| recent - before) {
        Some(change) if change > TREND_STEP_F => Trend::Rising,
        Some(change) if change < -TREND_STEP_F => Trend::Falling,
        Some(_) => Trend::Stable,
        None => Trend::Unknown,
    }
}
