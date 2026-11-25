//! Reading the owner's Home Assistant sensors on the schedule: every 5 minutes, each
//! sensor on its own, each outcome recorded as that sensor's status.

use super::order::Failing;
use super::sensor_store::{take, Taken, PRODUCT};
use crate::datasources::homeassistant::connection::{HaConnection, Unusable};
use crate::db::setup_queries::{self, Sensor, Service, WayIn};
use crate::db::weather_queries::sensors;
use crate::db::weather_queries::{fetch_state, sources as weather_sources};
use crate::error::Result;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use sqlx::PgPool;
use std::collections::HashSet;

/// How often the owner's sensors are read.
pub const READ_EVERY: Duration = Duration::minutes(5);

/// What a call to `read` did.
#[derive(Debug, Clone, PartialEq)]
pub enum Read {
    /// Less than 5 minutes since the last attempt, or waiting out a failure.
    NotDue,
    /// No connection is stored or no Home Assistant sensor is mapped: nothing was asked.
    NoSensors,
    /// Home Assistant did not answer or refused the token; every sensor's status says so.
    Failed(Unusable),
    /// Home Assistant answered. `announce` is set on the first such read after a start
    /// or a failure, which is logged at INFO.
    Done {
        sensors: usize,
        stored: usize,
        announce: bool,
    },
}

#[derive(Debug, Default)]
pub struct SensorReader {
    last_attempt: Option<DateTime<Utc>>,
    failing: Failing,
    announced: bool,
    pruned_on: Option<NaiveDate>,
    /// A refresh asked for the next read.
    forced: bool,
}

impl SensorReader {
    pub fn new() -> Self {
        Self::default()
    }

    /// The next read does not wait for the 5 minutes or a failure's retry (a refresh).
    // @spec SYNC-SCHED-009
    pub fn force(&mut self) {
        self.forced = true;
    }

    /// One pass's work on the owner's sensors: read them when due and drop readings older
    /// than a week once a day. Logged, never returned: it must not stop the pass.
    // @spec SYNC-SCHED-017, SYNC-SCHED-018
    /// Returns whether a reading was stored.
    pub async fn run(&mut self, pool: &PgPool, today: NaiveDate, now: DateTime<Utc>) -> bool {
        let stored_any = match self.read(pool, today, now).await {
            Ok(Read::Done {
                sensors,
                stored,
                announce: true,
            }) => {
                tracing::info!(sensors, stored, "Home Assistant sensors read");
                stored > 0
            }
            Ok(Read::Done {
                sensors, stored, ..
            }) => {
                tracing::debug!(sensors, stored, "Home Assistant sensors read");
                stored > 0
            }
            Ok(_) => false,
            Err(error) => {
                tracing::error!(?error, "reading the owner's sensors failed");
                false
            }
        };
        match self.prune(pool, today, now).await {
            Ok(Some(dropped)) if dropped > 0 => {
                tracing::debug!(dropped, "Sensor readings older than a week deleted")
            }
            Ok(_) => {}
            Err(error) => tracing::error!(?error, "deleting old sensor readings failed"),
        }
        stored_any
    }

    /// Reads the mapped Home Assistant sensors when they are due and stores what they
    /// report. `today` is the lawn's date.
    // @spec SYNC-SCHED-013, SYNC-SCHED-014, SYNC-SCHED-015, SYNC-SCHED-016, WX-SRC-HA-001
    pub async fn read(
        &mut self,
        pool: &PgPool,
        today: NaiveDate,
        now: DateTime<Utc>,
    ) -> Result<Read> {
        let waited = self.last_attempt.is_none_or(|at| now - at >= READ_EVERY);
        let forced = std::mem::take(&mut self.forced);
        if !forced && (!waited || !self.failing.may_try(now)) {
            return Ok(Read::NotDue);
        }
        let Some(held) = setup_queries::credential(pool, Service::HomeAssistant).await? else {
            return Ok(Read::NoSensors);
        };
        let mapped = mapped(pool, today).await?;
        if mapped.is_empty() {
            return Ok(Read::NoSensors);
        }
        self.last_attempt = Some(now);
        let address = held.address.as_deref().unwrap_or_default();
        let connection = match HaConnection::new(address, &held.secret) {
            Ok(connection) => connection,
            Err(unusable) => return self.failed(pool, &mapped, unusable, now).await,
        };
        let mut stored = 0;
        for (sensor, source_id) in &mapped {
            let entity_id = sensor.entity_id.as_deref().unwrap_or_default();
            let lookup = match connection.entity(entity_id).await {
                Ok(lookup) => lookup,
                Err(unusable) => return self.failed(pool, &mapped, unusable, now).await,
            };
            match take(pool, sensor, *source_id, lookup, now).await? {
                Taken::Stored => stored += 1,
                Taken::Refused(said) => {
                    tracing::warn!(sensor = %sensor.name, problem = %said, "Sensor reading not used")
                }
                Taken::Starting | Taken::Unchanged => {}
            }
        }
        self.failing.succeeded();
        let announce = !self.announced;
        self.announced = true;
        Ok(Read::Done {
            sensors: mapped.len(),
            stored,
            announce,
        })
    }

    /// Home Assistant could not be used: every sensor's status says why, and the next
    /// read waits out the failure.
    async fn failed(
        &mut self,
        pool: &PgPool,
        mapped: &[(Sensor, i64)],
        unusable: Unusable,
        now: DateTime<Utc>,
    ) -> Result<Read> {
        self.failing.failed(now);
        self.announced = false;
        tracing::warn!(
            problem = unusable.sentence(),
            "Home Assistant sensors not read"
        );
        for (_, source_id) in mapped {
            fetch_state::failed(pool, *source_id, PRODUCT, unusable.sentence(), now).await?;
        }
        Ok(Read::Failed(unusable))
    }

    /// Deletes sensor readings older than the record keeps them, once a day. Returns how
    /// many were deleted, or `None` when it already ran today.
    // @spec SYNC-SCHED-018
    pub async fn prune(
        &mut self,
        pool: &PgPool,
        today: NaiveDate,
        now: DateTime<Utc>,
    ) -> Result<Option<u64>> {
        if self.pruned_on == Some(today) {
            return Ok(None);
        }
        let dropped = sensors::prune_readings(pool, now).await?;
        self.pruned_on = Some(today);
        Ok(Some(dropped))
    }
}

/// The Home Assistant sensors that are mapped and in the plan, with their sources, each
/// registered so its status can be recorded.
async fn mapped(pool: &PgPool, today: NaiveDate) -> Result<Vec<(Sensor, i64)>> {
    let sources = weather_sources::all(pool).await?;
    let registered: HashSet<i64> = fetch_state::all(pool)
        .await?
        .into_iter()
        .map(|state| state.source_id)
        .collect();
    let mut mapped = Vec::new();
    for sensor in setup_queries::sensors(pool).await? {
        if sensor.way_in != WayIn::HomeAssistant {
            continue;
        }
        let key = sensor.source_key();
        let Some(source) = sources.iter().find(|source| source.key == key) else {
            continue;
        };
        if !registered.contains(&source.id) {
            fetch_state::want(pool, source.id, PRODUCT, today).await?;
        }
        mapped.push((sensor, source.id));
    }
    Ok(mapped)
}
