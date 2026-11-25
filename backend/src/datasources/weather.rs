use crate::config::DataLakeConfig;
use crate::error::{Result, Source, TurfOpsError};
use crate::models::{celsius_to_fahrenheit, DataSource, EnvironmentalReading};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
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

    /// Hourly readings in [start, end], newest first (matches the old ordering the
    /// 7-day summary + trend logic relies on).
    pub async fn fetch_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<EnvironmentalReading>> {
        let src = Self::parquet(&self.silver_weather_path);
        let station = self.station_wbanno;
        let (start_s, end_s) = (fmt_ts(start), fmt_ts(end));
        Self::run(move |conn| {
            let sql = format!(
                "SELECT obs_ts_utc, soil_temp_5, soil_temp_10, soil_temp_20, soil_temp_50, soil_temp_100, \
                        soil_moisture_5, soil_moisture_10, soil_moisture_20, soil_moisture_50, soil_moisture_100, \
                        air_temp_c, rh_pct, precip_mm \
                 FROM {src} \
                 WHERE CAST(wbanno AS INTEGER) = ? AND obs_ts_utc >= ?::TIMESTAMP AND obs_ts_utc <= ?::TIMESTAMP \
                 ORDER BY obs_ts_utc DESC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query(duckdb::params![station, start_s, end_s])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(row_to_reading(row)?);
            }
            Ok(out)
        })
        .await
    }

    /// Daily GDD rows `(date, high_f, low_f, gdd50)` from the gold layer for [start, end].
    /// `gdd50` is precomputed in the lake with the identical base-50°F formula.
    // @spec WX-SRC-LAKE-008
    pub async fn fetch_daily_gdd(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<DailyGddRow>> {
        let src = Self::parquet(&self.gold_weather_path);
        let (start_s, end_s) = (start.to_string(), end.to_string());
        Self::run(move |conn| {
            let sql = format!(
                "SELECT day, air_temp_max_f, air_temp_min_f, gdd50 \
                 FROM {src} \
                 WHERE day >= ?::DATE AND day <= ?::DATE AND gdd50 IS NOT NULL \
                 ORDER BY day ASC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query(duckdb::params![start_s, end_s])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                let date: NaiveDate = row.get(0)?;
                // air_temp_max/min can be NULL on sparse days even when gdd50 is present;
                // they only feed the high/low display, so default to 0.0 when missing.
                let high_f: f64 = row.get::<_, Option<f64>>(1)?.unwrap_or(0.0);
                let low_f: f64 = row.get::<_, Option<f64>>(2)?.unwrap_or(0.0);
                let gdd50: f64 = row.get::<_, Option<f64>>(3)?.unwrap_or(0.0);
                out.push((date, high_f, low_f, gdd50));
            }
            Ok(out)
        })
        .await
    }
}

/// Format a UTC instant as a naive `YYYY-MM-DD HH:MM:SS` string for a TIMESTAMP comparison.
fn fmt_ts(ts: DateTime<Utc>) -> String {
    ts.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Map a silver hourly row to an `EnvironmentalReading`, converting °C → °F.
/// Column order must match the SELECT lists above.
// @spec WX-SRC-LAKE-003
fn row_to_reading(row: &duckdb::Row) -> duckdb::Result<EnvironmentalReading> {
    let ts: NaiveDateTime = row.get(0)?;
    let mut reading = EnvironmentalReading::new(DataSource::SoilData);
    reading.timestamp = DateTime::<Utc>::from_naive_utc_and_offset(ts, Utc);

    // `> -50 °C` drops any raw USCRN missing-data sentinel (-9999) that survived cleaning,
    // as the climatology read does; one such hour would move a 7-day mean by ~100 °F.
    let temp_f = |c: Option<f64>| c.filter(|v| *v > -50.0).map(celsius_to_fahrenheit);
    reading.soil_temp_5_f = temp_f(row.get(1)?);
    reading.soil_temp_10_f = temp_f(row.get(2)?);
    reading.soil_temp_20_f = temp_f(row.get(3)?);
    reading.soil_temp_50_f = temp_f(row.get(4)?);
    reading.soil_temp_100_f = temp_f(row.get(5)?);

    let moisture = |m: Option<f64>| m.filter(|v| *v >= 0.0);
    reading.soil_moisture_5 = moisture(row.get(6)?);
    reading.soil_moisture_10 = moisture(row.get(7)?);
    reading.soil_moisture_20 = moisture(row.get(8)?);
    reading.soil_moisture_50 = moisture(row.get(9)?);
    reading.soil_moisture_100 = moisture(row.get(10)?);

    reading.ambient_temp_f = temp_f(row.get(11)?);
    reading.humidity_percent = row.get::<_, Option<f64>>(12)?.filter(|h| *h >= 0.0);
    reading.precipitation_mm = row.get::<_, Option<f64>>(13)?.filter(|p| *p >= 0.0);

    Ok(reading)
}
