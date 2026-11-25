//! Test helpers for data sync: a summary for pages that still read the lake, a held
//! forecast, and forecast fixtures.

use super::*;
#[cfg(feature = "db-tests")]
use crate::models::{DailyForecast, ForecastProvider, WeatherCondition, WeatherForecast};
#[cfg(feature = "db-tests")]
use chrono::NaiveDate;
#[cfg(feature = "db-tests")]
use chrono::Utc;

impl DataSyncService {
    /// The same service answering with the lake's summary, for pages that still read
    /// the lake (tests).
    pub(crate) fn summary_from_lake(mut self) -> Self {
        self.lake_summary = true;
        self
    }

    /// Answer with `summary` (and the forecast held), for pages that still read the lake
    /// (tests).
    #[cfg(feature = "db-tests")]
    pub(crate) fn pin_summary(&mut self, summary: EnvironmentalSummary) {
        self.lake_summary = true;
        self.pinned = Some(summary);
    }

    /// Read the forecast from `slot` from now on.
    #[cfg(feature = "db-tests")]
    pub(crate) fn use_forecast(&mut self, slot: ForecastSlot) {
        self.forecast = slot;
    }
}

#[cfg(feature = "db-tests")]
/// A forecast fetched now with the given daily summaries and no hourly points.
pub(crate) fn forecast(daily_summary: Vec<DailyForecast>) -> WeatherForecast {
    WeatherForecast {
        fetched_at: Utc::now(),
        provider: ForecastProvider::OpenWeatherMap,
        hourly: vec![],
        daily_summary,
    }
}

#[cfg(feature = "db-tests")]
/// A forecast day with the given coverage and high / low.
pub(crate) fn forecast_day(
    date: NaiveDate,
    hours_covered: u32,
    high_temp_f: f64,
    low_temp_f: f64,
) -> DailyForecast {
    DailyForecast {
        date,
        high_temp_f,
        low_temp_f,
        avg_humidity: 60.0,
        total_precipitation_mm: Some(0.0),
        max_precipitation_prob: Some(0.0),
        dominant_condition: WeatherCondition::Clear,
        avg_wind_speed_mph: 5.0,
        max_wind_gust_mph: None,
        hours_covered,
    }
}
