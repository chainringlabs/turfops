//! Test helpers for data sync: a fixed summary in place of the record's, a held forecast,
//! and forecast fixtures.

use super::*;
#[cfg(feature = "db-tests")]
use crate::models::{DailyForecast, ForecastProvider, WeatherCondition, WeatherForecast};
#[cfg(feature = "db-tests")]
use chrono::NaiveDate;
#[cfg(feature = "db-tests")]
use chrono::Utc;

/// A summary a test fixes in place of the one assembled from the weather record.
#[derive(Clone)]
pub(crate) enum Fixed {
    /// No weather: every weather field empty, with whatever forecast is held.
    #[cfg_attr(not(feature = "db-tests"), allow(dead_code))]
    NoWeather,
    /// The summary given, or a reading stamped at the Unix epoch (so two calls compare
    /// equal), with the forecast held while it is fresh.
    Reading(Option<Box<EnvironmentalSummary>>),
}

impl DataSyncService {
    /// A service over `pool` answering with `fixed`, with no forecast and no schedule.
    #[cfg(feature = "db-tests")]
    pub(crate) fn for_tests(pool: PgPool, clock: Clock, fixed: Fixed) -> Self {
        Self {
            pool,
            forecast: ForecastSlot::new(),
            waker: None,
            kept: Default::default(),
            refreshed_at: None,
            clock,
            fixed: Some(fixed),
        }
    }

    /// The same service answering with a fixed reading instead of the record's summary.
    pub(crate) fn fixed_reading(mut self) -> Self {
        self.fixed = Some(Fixed::Reading(None));
        self
    }

    /// Answer with `summary` (and the forecast held) instead of the record's.
    #[cfg(feature = "db-tests")]
    pub(crate) fn pin_summary(&mut self, summary: EnvironmentalSummary) {
        self.fixed = Some(Fixed::Reading(Some(Box::new(summary))));
    }

    /// The summary `fixed` stands for, now.
    pub(super) fn fixed_summary(&self, fixed: &Fixed) -> EnvironmentalSummary {
        let now = self.clock.now();
        let mut summary = match fixed {
            Fixed::NoWeather => {
                return EnvironmentalSummary {
                    last_updated: Some(now),
                    forecast: self.forecast.forecast(),
                    ..Default::default()
                }
            }
            Fixed::Reading(Some(pinned)) => (**pinned).clone(),
            Fixed::Reading(None) => EnvironmentalSummary {
                current: Some(crate::models::EnvironmentalReading {
                    timestamp: DateTime::UNIX_EPOCH,
                    ..Default::default()
                }),
                ..Default::default()
            },
        };
        summary.forecast = self
            .forecast
            .forecast()
            .filter(|f| now - f.fetched_at < assemble::FORECAST_MAX_AGE);
        summary.last_updated = Some(now);
        summary
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
