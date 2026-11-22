//! The owner's sensor readings: kept as received for a week, and folded into the sensor
//! source's hourly row as they arrive.

use crate::error::{Result, TurfOpsError};
use crate::models::weather_record::Input;
use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::PgPool;

/// Raw readings older than this are dropped; the hourly rows they made stay.
pub const READINGS_KEPT: Duration = Duration::days(7);

/// The statement that recomputes one hour of one hourly column from that hour's readings.
/// Each is a fixed string: the column and the aggregate are never built from request text.
macro_rules! fold {
    ($column:literal, $aggregate:literal) => {
        concat!(
            "INSERT INTO weather_hourly (source_id, observed_at, ",
            $column,
            ") SELECT $1, $3, ",
            $aggregate,
            "(value) FROM weather_sensor_reading \
             WHERE source_id = $1 AND input = $2 AND observed_at >= $3 \
               AND observed_at < $3 + INTERVAL '1 hour' \
             ON CONFLICT (source_id, observed_at) DO UPDATE SET ",
            $column,
            " = EXCLUDED.",
            $column
        )
    };
}

/// An hour's value is the total of its readings for rain and their mean for everything
/// else. `DailyAir` is not something a sensor measures.
fn fold_statement(input: Input) -> Option<&'static str> {
    Some(match input {
        Input::AirTemp => fold!("air_temp_f", "AVG"),
        Input::Humidity => fold!("rh_pct", "AVG"),
        Input::Rain => fold!("precip_mm", "SUM"),
        Input::SoilTemp5 => fold!("soil_temp_5_f", "AVG"),
        Input::SoilTemp10 => fold!("soil_temp_10_f", "AVG"),
        Input::SoilTemp20 => fold!("soil_temp_20_f", "AVG"),
        Input::SoilTemp50 => fold!("soil_temp_50_f", "AVG"),
        Input::SoilTemp100 => fold!("soil_temp_100_f", "AVG"),
        Input::SoilMoisture5 => fold!("soil_moisture_5", "AVG"),
        Input::SoilMoisture10 => fold!("soil_moisture_10", "AVG"),
        Input::SoilMoisture20 => fold!("soil_moisture_20", "AVG"),
        Input::SoilMoisture50 => fold!("soil_moisture_50", "AVG"),
        Input::SoilMoisture100 => fold!("soil_moisture_100", "AVG"),
        Input::DailyAir => return None,
    })
}

/// Stores a reading as received and recomputes its hour of the sensor source's hourly
/// row from every reading in that hour: the total for rain, the mean for everything else.
// @spec WX-REC-STORE-006
pub async fn record_reading(
    pool: &PgPool,
    source_id: i64,
    input: Input,
    observed_at: DateTime<Utc>,
    value: f64,
) -> Result<()> {
    let fold = fold_statement(input).ok_or_else(|| {
        TurfOpsError::Internal(format!("a sensor cannot be mapped to {}", input.as_str()))
    })?;
    let hour = observed_at
        .duration_trunc(Duration::hours(1))
        .map_err(|e| TurfOpsError::Internal(format!("reading time: {e}")))?;
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"INSERT INTO weather_sensor_reading (source_id, input, observed_at, value)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (source_id, input, observed_at) DO UPDATE SET value = EXCLUDED.value"#,
    )
    .bind(source_id)
    .bind(input.as_str())
    .bind(observed_at)
    .bind(value)
    .execute(&mut *tx)
    .await?;
    sqlx::query(fold)
        .bind(source_id)
        .bind(input.as_str())
        .bind(hour)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Drops raw readings older than `READINGS_KEPT`; returns how many. Hourly rows are untouched.
// @spec WX-REC-STORE-007
pub async fn prune_readings(pool: &PgPool, now: DateTime<Utc>) -> Result<u64> {
    let dropped = sqlx::query("DELETE FROM weather_sensor_reading WHERE observed_at < $1")
        .bind(now - READINGS_KEPT)
        .execute(pool)
        .await?;
    Ok(dropped.rows_affected())
}

/// A sensor's newest raw reading of an input, with when it was observed.
pub async fn latest_reading(
    pool: &PgPool,
    source_id: i64,
    input: Input,
) -> Result<Option<(DateTime<Utc>, f64)>> {
    Ok(sqlx::query_as(
        "SELECT observed_at, value FROM weather_sensor_reading \
         WHERE source_id = $1 AND input = $2 ORDER BY observed_at DESC LIMIT 1",
    )
    .bind(source_id)
    .bind(input.as_str())
    .fetch_optional(pool)
    .await?)
}
