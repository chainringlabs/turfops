//! What the owner has chosen about the lawn's weather sources: mapped sensors, pinned
//! stations, the forecast provider and stored connections.

use crate::error::{Result, TurfOpsError};
use crate::models::forecast::ForecastProvider;
use crate::models::weather_record::Input;
use sqlx::PgPool;

mod apply;
mod credentials;

pub use apply::{apply, Applied};
// `credential_ever_stored` is read by the tests of forgetting and importing.
#[allow(unused_imports)]
pub use credentials::{
    credential, credential_ever_stored, forget_credential, import_once, store_credential,
    Credential, Service,
};

/// How a sensor's readings reach TurfOps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WayIn {
    HomeAssistant,
    Posted,
}

/// What a rain sensor's number means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RainKind {
    /// A total that only rises; the rain is its increase.
    Counter,
    /// The rain since its last reading.
    Amount,
}

/// One of the owner's sensors that is mapped to an input.
#[derive(Debug, Clone, PartialEq)]
pub struct Sensor {
    pub id: i64,
    pub name: String,
    pub way_in: WayIn,
    pub entity_id: Option<String>,
    pub input: Input,
    pub rain_kind: Option<RainKind>,
}

impl Sensor {
    /// The key of the weather source that holds this sensor's readings.
    pub fn source_key(&self) -> String {
        format!("sensor:{}", self.id)
    }
}

/// A sensor to be mapped.
#[derive(Debug, Clone, PartialEq)]
pub struct NewSensor {
    pub name: String,
    pub entity_id: Option<String>,
    pub input: Input,
    pub rain_kind: Option<RainKind>,
}

type SensorRow = (i64, String, String, Option<String>, String, Option<String>);

fn sensor(row: SensorRow) -> Result<Sensor> {
    let (id, name, way_in, entity_id, input, rain_kind) = row;
    let broken = |what: &str| TurfOpsError::Internal(format!("sensor {id} has an unknown {what}"));
    Ok(Sensor {
        id,
        name,
        way_in: match way_in.as_str() {
            "HomeAssistant" => WayIn::HomeAssistant,
            "Posted" => WayIn::Posted,
            _ => return Err(broken("way in")),
        },
        entity_id,
        input: Input::parse(&input).ok_or_else(|| broken("input"))?,
        rain_kind: match rain_kind.as_deref() {
            None => None,
            Some("Counter") => Some(RainKind::Counter),
            Some("Amount") => Some(RainKind::Amount),
            Some(_) => return Err(broken("rain kind")),
        },
    })
}

/// The sensors that are mapped, oldest first.
pub async fn sensors(pool: &PgPool) -> Result<Vec<Sensor>> {
    let rows: Vec<SensorRow> = sqlx::query_as(
        "SELECT id, name, way_in, entity_id, input, rain_kind FROM sensor \
         WHERE unmapped_at IS NULL ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(sensor).collect()
}

/// Maps a sensor. One already mapped to the same input, or the same Home Assistant
/// entity already mapped, is a conflict.
// @spec SETUP-MAP-003, SETUP-MAP-008
pub async fn map_sensor(pool: &PgPool, new: &NewSensor) -> Result<Sensor> {
    let way_in = match new.entity_id {
        Some(_) => "HomeAssistant",
        None => "Posted",
    };
    let rain_kind = new.rain_kind.map(|kind| match kind {
        RainKind::Counter => "Counter",
        RainKind::Amount => "Amount",
    });
    let row: SensorRow = sqlx::query_as(
        "INSERT INTO sensor (name, way_in, entity_id, input, rain_kind) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, name, way_in, entity_id, input, rain_kind",
    )
    .bind(&new.name)
    .bind(way_in)
    .bind(&new.entity_id)
    .bind(new.input.as_str())
    .bind(rain_kind)
    .fetch_one(pool)
    .await?;
    sensor(row)
}

fn rain_name(kind: Option<RainKind>) -> Option<&'static str> {
    kind.map(|kind| match kind {
        RainKind::Counter => "Counter",
        RainKind::Amount => "Amount",
    })
}

/// Changes a mapped sensor's name and what it is mapped to; a change of input or rain
/// kind discards its previous reading. `None` when no mapped sensor has that id.
// @spec WX-SRC-HA-012
pub async fn change_sensor(
    pool: &PgPool,
    id: i64,
    name: &str,
    input: Input,
    rain_kind: Option<RainKind>,
) -> Result<Option<Sensor>> {
    let row: Option<SensorRow> = sqlx::query_as(
        // A value in one input's units says nothing about another's (WX-SRC-HA-012).
        "UPDATE sensor SET name = $2, input = $3, rain_kind = $4, \
           previous_value = CASE WHEN input = $3 AND rain_kind IS NOT DISTINCT FROM $4 \
                            THEN previous_value END, \
           previous_at = CASE WHEN input = $3 AND rain_kind IS NOT DISTINCT FROM $4 \
                         THEN previous_at END \
         WHERE id = $1 AND unmapped_at IS NULL \
         RETURNING id, name, way_in, entity_id, input, rain_kind",
    )
    .bind(id)
    .bind(name)
    .bind(input.as_str())
    .bind(rain_name(rain_kind))
    .fetch_optional(pool)
    .await?;
    row.map(sensor).transpose()
}

/// Whether any sensor was ever mapped, including ones since unmapped.
pub async fn any_sensor_ever(pool: &PgPool) -> Result<bool> {
    Ok(sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM sensor)")
        .fetch_one(pool)
        .await?)
}

/// Unmaps a sensor, keeping it and the readings stored under it. Returns whether a
/// mapped sensor had that id.
// @spec SETUP-MAP-009
pub async fn unmap_sensor(pool: &PgPool, id: i64) -> Result<bool> {
    let done =
        sqlx::query("UPDATE sensor SET unmapped_at = NOW() WHERE id = $1 AND unmapped_at IS NULL")
            .bind(id)
            .execute(pool)
            .await?;
    Ok(done.rows_affected() > 0)
}

/// Each pinned input with its station's catalog row.
pub async fn pins(pool: &PgPool) -> Result<Vec<(Input, i64)>> {
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT input, station_id FROM station_pin ORDER BY input")
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(input, station)| Some((Input::parse(&input)?, station)))
        .collect())
}

/// Pins a station for an input, in place of any pinned before.
pub async fn pin(pool: &PgPool, input: Input, station_ref: i64) -> Result<()> {
    sqlx::query(
        "INSERT INTO station_pin (input, station_id) VALUES ($1, $2) \
         ON CONFLICT (input) DO UPDATE SET station_id = EXCLUDED.station_id, pinned_at = NOW()",
    )
    .bind(input.as_str())
    .bind(station_ref)
    .execute(pool)
    .await?;
    Ok(())
}

/// Removes an input's pin. Returns whether there was one.
pub async fn unpin(pool: &PgPool, input: Input) -> Result<bool> {
    let done = sqlx::query("DELETE FROM station_pin WHERE input = $1")
        .bind(input.as_str())
        .execute(pool)
        .await?;
    Ok(done.rows_affected() > 0)
}

pub(super) fn provider_name(provider: ForecastProvider) -> &'static str {
    match provider {
        ForecastProvider::OpenMeteo => "OpenMeteo",
        ForecastProvider::NationalWeatherService => "NationalWeatherService",
        ForecastProvider::OpenWeatherMap => "OpenWeatherMap",
    }
}

/// The forecast provider the owner chose; Open-Meteo until they choose.
// @spec WX-SRC-FC-001
pub async fn forecast_provider(pool: &PgPool) -> Result<ForecastProvider> {
    let chosen: Option<String> =
        sqlx::query_scalar("SELECT forecast_provider FROM weather_setting")
            .fetch_optional(pool)
            .await?;
    Ok(match chosen.as_deref() {
        Some("NationalWeatherService") => ForecastProvider::NationalWeatherService,
        Some("OpenWeatherMap") => ForecastProvider::OpenWeatherMap,
        _ => ForecastProvider::OpenMeteo,
    })
}

pub async fn set_forecast_provider(pool: &PgPool, provider: ForecastProvider) -> Result<()> {
    sqlx::query(
        "INSERT INTO weather_setting (id, forecast_provider) VALUES (TRUE, $1) \
         ON CONFLICT (id) DO UPDATE SET forecast_provider = EXCLUDED.forecast_provider",
    )
    .bind(provider_name(provider))
    .execute(pool)
    .await?;
    Ok(())
}

/// What a sensor named in the environment measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapped {
    AirTemp,
    Humidity,
}

/// On an installation where no sensor was ever mapped, maps the Home Assistant entities
/// the environment names. Returns the names of the sensors it mapped.
// @spec SETUP-MAP-011
pub async fn map_first_sensors(
    pool: &PgPool,
    entities: &[(&'static str, &str, Mapped)],
) -> Result<Vec<&'static str>> {
    if any_sensor_ever(pool).await? {
        return Ok(Vec::new());
    }
    let mut mapped = Vec::new();
    for (name, entity_id, measures) in entities {
        if entity_id.trim().is_empty() {
            continue;
        }
        let new = NewSensor {
            name: (*name).to_string(),
            entity_id: Some(entity_id.trim().to_string()),
            input: match measures {
                Mapped::AirTemp => Input::AirTemp,
                Mapped::Humidity => Input::Humidity,
            },
            rain_kind: None,
        };
        map_sensor(pool, &new).await?;
        mapped.push(*name);
    }
    Ok(mapped)
}
