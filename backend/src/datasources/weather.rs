use crate::config::DataLakeConfig;
use crate::error::{Result, Source, TurfOpsError};

use chrono::NaiveDate;
use duckdb::Connection;

mod climatology;
mod disease;
#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod silver_tests;
#[cfg(test)]
mod window_tests;

pub use climatology::{ClimateDay, ClimateRecord};

/// Reads NOAA USCRN weather data from the Dagster data lake (parquet on a mounted
/// filesystem) using an embedded DuckDB engine.
///
/// Replaces the retired SoilData PostgreSQL connection. Two layers are used:
/// - **silver** (`silver_weather_path`): hourly, cleaned + deduped observations in
///   native units (°C, mm, % RH, fractional moisture). Drives the live reading,
///   the 7-day rolling summary, and the soil-temp trend.
/// - **gold** (`gold_weather_path`): daily pre-aggregated means already in °F, plus a
///   precomputed `gdd50`. Drives the seasonal plan, the soil-temp regression, and GDD.
///
/// DuckDB connections are not `Sync`, so every query opens a short-lived in-memory
/// connection inside `spawn_blocking` and scans the parquet directly.
#[derive(Clone)]
pub struct WeatherLakeClient {
    silver_weather_path: String,
    gold_weather_path: String,
    station_wbanno: i32,
}

/// One daily GDD record sourced from the gold layer, before cumulative accumulation.
/// `(date, air_temp_max_f, air_temp_min_f, gdd50)`.
pub type DailyGddRow = (NaiveDate, f64, f64, f64);

impl WeatherLakeClient {
    pub fn new(config: &DataLakeConfig, station_wbanno: i32) -> Self {
        Self {
            silver_weather_path: config.silver_weather_path.clone(),
            gold_weather_path: config.gold_weather_path.clone(),
            station_wbanno,
        }
    }

    /// Wrap a blocking DuckDB closure in `spawn_blocking` and normalize errors.
    // @spec WX-SRC-LAKE-001
    async fn run<T, F>(f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
    {
        tokio::task::spawn_blocking(move || {
            let conn = Connection::open_in_memory()?;
            f(&conn)
        })
        .await
        .map_err(|e| {
            TurfOpsError::unavailable(Source::WeatherLake, format!("data lake task failed: {e}"))
        })?
    }

    // @spec WX-SRC-LAKE-002
    /// `read_parquet('<path>')` with single quotes escaped (paths come from trusted config).
    fn parquet(path: &str) -> String {
        format!("read_parquet('{}')", path.replace('\'', "''"))
    }
}
