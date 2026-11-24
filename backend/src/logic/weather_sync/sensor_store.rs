//! Taking one reading of one of the owner's sensors, whichever way it came in: judged,
//! stored with the sensor's new previous reading, and recorded as the sensor's status.

use super::sensor_reading::{judge, Judged, Previous};
use crate::datasources::cadence::Product;
use crate::datasources::homeassistant::connection::Lookup;
use crate::db::setup_queries::Sensor;
use crate::db::weather_queries::sensors::{self, NewReading};
use crate::db::weather_queries::{fetch_state, sources};
use crate::error::Result;
use crate::logic::weather_record::plan::sensor_source;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;

/// The product a sensor's status is recorded under.
pub const PRODUCT: &str = Product::SensorEntities.as_str();

/// What became of one reading.
#[derive(Debug, Clone, PartialEq)]
pub enum Taken {
    /// Written to the record.
    Stored,
    /// A rain counter's first reading, kept as its starting point.
    Starting,
    /// Not later than the sensor's previous reading.
    Unchanged,
    /// Not used, for the reason given (the sentence recorded as the sensor's status).
    Refused(String),
}

/// The id of the weather source that holds `sensor`'s readings, created when it has
/// none yet (a lawn with no location has no plan to create it), with the sensor's status
/// registered.
// @spec WX-SRC-POST-013
pub async fn source_for(pool: &PgPool, sensor: &Sensor, today: NaiveDate) -> Result<i64> {
    let source = sources::ensure(pool, &sensor_source(&sensor.source_key(), &sensor.name)).await?;
    let registered = fetch_state::all(pool)
        .await?
        .iter()
        .any(|state| state.source_id == source.id);
    if !registered {
        fetch_state::want(pool, source.id, PRODUCT, today).await?;
    }
    Ok(source.id)
}

/// Judges one reading of `sensor`, stores what it gives together with the sensor's new
/// previous reading, and records the outcome as the sensor's status.
// @spec WX-SRC-HA-011, SYNC-SCHED-015, WX-SRC-POST-006, WX-SRC-POST-009, WX-SRC-POST-011
pub async fn take(
    pool: &PgPool,
    sensor: &Sensor,
    source_id: i64,
    lookup: Lookup,
    now: DateTime<Utc>,
) -> Result<Taken> {
    let previous = sensors::previous_reading(pool, sensor.id)
        .await?
        .map(|(value, at)| Previous { value, at });
    let kept = |keep: Previous| (keep.value, keep.at);
    let taken = match judge(sensor.input, sensor.rain_kind, lookup, previous, now) {
        Judged::Store {
            at,
            value,
            keep,
            jumped,
        } => {
            if jumped {
                tracing::warn!(
                    sensor = %sensor.name,
                    "Rain counter rose more than 300 mm in one reading; counted as no rain"
                );
            }
            let reading = NewReading {
                source_id,
                input: sensor.input,
                observed_at: at,
                value,
            };
            sensors::keep_reading(pool, sensor.id, kept(keep), Some(reading)).await?;
            Taken::Stored
        }
        Judged::Starting { keep } => {
            sensors::keep_reading(pool, sensor.id, kept(keep), None).await?;
            Taken::Starting
        }
        Judged::Unchanged => Taken::Unchanged,
        Judged::Refused(problem) => {
            let entity_id = sensor.entity_id.as_deref().unwrap_or_default();
            let said = problem.sentence(sensor.input, entity_id);
            refused(pool, source_id, &said, now).await?;
            return Ok(Taken::Refused(said));
        }
    };
    fetch_state::checked(pool, source_id, PRODUCT, now).await?;
    Ok(taken)
}

/// Records a reading that was not used as the sensor's status.
pub async fn refused(pool: &PgPool, source_id: i64, said: &str, now: DateTime<Utc>) -> Result<()> {
    fetch_state::failed(pool, source_id, PRODUCT, said, now).await
}
