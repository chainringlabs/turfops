//! A data sync service and app state for handler tests: a lake client over test parquet,
//! no Home Assistant client, and an optional forecast held as the schedule would hold it. The database pool is lazy and never connects.

use super::*;
use crate::models::{DailyForecast, ForecastProvider, WeatherCondition, WeatherForecast};
use crate::state::AppState;
use chrono::NaiveDate;
use chrono::Utc;

impl DataSyncService {
    pub(crate) fn for_tests(
        weather_client: Option<WeatherLakeClient>,
        forecast: Option<WeatherForecast>,
        clock: Clock,
    ) -> Self {
        let forecast = forecast.map(ForecastSlot::holding).unwrap_or_default();
        Self {
            pool: lazy_pool(),
            weather_client,
            homeassistant_client: None,
            forecast,
            state: SyncState::default(),
            probes: Mutex::new(ProbeCache::default()),
            clock,
        }
    }

    /// Read the forecast from `slot` from now on.
    #[cfg(feature = "db-tests")]
    pub(crate) fn use_forecast(&mut self, slot: ForecastSlot) {
        self.forecast = slot;
    }

    pub(crate) fn has_home_assistant(&self) -> bool {
        self.homeassistant_client.is_some()
    }
}

/// A pool that is never used: nothing listens at its address.
pub(crate) fn lazy_pool() -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1:1/unused")
        .unwrap()
}

/// The lawn's clock in tests.
pub(crate) fn eastern() -> Clock {
    Clock::Zone(chrono_tz::America::New_York)
}

/// A forecast fetched now with the given daily summaries and no hourly points.
pub(crate) fn forecast(daily_summary: Vec<DailyForecast>) -> WeatherForecast {
    WeatherForecast {
        fetched_at: Utc::now(),
        provider: ForecastProvider::OpenWeatherMap,
        hourly: vec![],
        daily_summary,
    }
}

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

/// App state over the given lake client and forecast.
pub(crate) fn app_state(
    weather_client: Option<WeatherLakeClient>,
    forecast: Option<WeatherForecast>,
) -> AppState {
    AppState::new(
        lazy_pool(),
        DataSyncService::for_tests(weather_client, forecast, eastern()),
        None,
        false,
        eastern(),
    )
}
