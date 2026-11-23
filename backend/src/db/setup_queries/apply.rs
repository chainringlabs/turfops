//! Setup's one save: a location, a set of pins and a forecast provider, all or none.

use super::credentials::Service;
use super::provider_name;
use crate::error::Result;
use crate::models::forecast::ForecastProvider;
use crate::models::weather_record::Input;
use crate::models::LawnLocation;
use sqlx::{PgConnection, PgPool};

/// What setup saves in one step. Any part may be absent.
#[derive(Debug, Clone, Default)]
pub struct Applied {
    pub location: Option<LawnLocation>,
    /// The whole set of pins, replacing those held.
    pub pins: Option<Vec<(Input, i64)>>,
    pub forecast_provider: Option<ForecastProvider>,
    pub openweathermap_key: Option<String>,
}

/// Saves a location, a set of pins and a forecast provider together or not at all.
/// Returns whether there was a lawn profile to save a location to.
// @spec SETUP-API-012
pub async fn apply(pool: &PgPool, applied: &Applied) -> Result<bool> {
    let mut tx = pool.begin().await?;
    if let Some(location) = &applied.location {
        if !save_location(&mut tx, location).await? {
            return Ok(false);
        }
    }
    save_choices(&mut tx, applied).await?;
    tx.commit().await?;
    Ok(true)
}

/// Writes the lawn's location. The elevation belongs to the point, so a lawn that moves
/// has none until its weather is fetched again. Returns whether there was a profile.
async fn save_location(conn: &mut PgConnection, location: &LawnLocation) -> Result<bool> {
    let saved = sqlx::query(
        "UPDATE lawn_profiles SET \
                 elevation_ft = CASE WHEN latitude IS DISTINCT FROM $1 \
                                       OR longitude IS DISTINCT FROM $2 \
                                     THEN NULL ELSE elevation_ft END, \
                 latitude = $1, longitude = $2, place_name = $3, timezone = $4, \
                 updated_at = NOW() \
             WHERE id = (SELECT id FROM lawn_profiles ORDER BY id LIMIT 1)",
    )
    .bind(location.latitude)
    .bind(location.longitude)
    .bind(&location.place_name)
    .bind(&location.timezone)
    .execute(&mut *conn)
    .await?;
    Ok(saved.rows_affected() > 0)
}

/// Writes the pins, the forecast provider and its key, whichever are given.
async fn save_choices(tx: &mut PgConnection, applied: &Applied) -> Result<()> {
    if let Some(pins) = &applied.pins {
        sqlx::query("DELETE FROM station_pin")
            .execute(&mut *tx)
            .await?;
        for (input, station_ref) in pins {
            sqlx::query("INSERT INTO station_pin (input, station_id) VALUES ($1, $2)")
                .bind(input.as_str())
                .bind(station_ref)
                .execute(&mut *tx)
                .await?;
        }
    }
    if let Some(provider) = applied.forecast_provider {
        sqlx::query(
            "INSERT INTO weather_setting (id, forecast_provider) VALUES (TRUE, $1) \
             ON CONFLICT (id) DO UPDATE SET forecast_provider = EXCLUDED.forecast_provider",
        )
        .bind(provider_name(provider))
        .execute(&mut *tx)
        .await?;
    }
    if let Some(key) = &applied.openweathermap_key {
        sqlx::query(
            "INSERT INTO service_credential (service, address, secret) VALUES ($1, NULL, $2) \
             ON CONFLICT (service) DO UPDATE SET secret = EXCLUDED.secret, set_at = NOW()",
        )
        .bind(Service::OpenWeatherMap.name())
        .bind(key)
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}
