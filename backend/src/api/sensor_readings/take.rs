//! Taking the usable readings of one request, each against its sensor.

use super::{Answer, Rejected, Taken as Listed};
use crate::datasources::homeassistant::connection::{Entity, Lookup};
use crate::db::setup_queries::{self, Sensor, WayIn};
use crate::error::Result;
use crate::logic::weather_sync::posted::{
    in_time_order, same_name, BadReading, PostedLog, PostedReading,
};
use crate::logic::weather_sync::sensor_store::{source_for, take, Taken};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use std::collections::HashMap;

/// Takes each reading of a request: oldest first, each against the one posted sensor its
/// name matches. `today` is the lawn's date and `now` when the request arrived.
// @spec WX-SRC-POST-004, WX-SRC-POST-007, WX-SRC-POST-008, WX-SRC-POST-009, WX-SRC-POST-012
pub(super) async fn take_all(
    pool: &PgPool,
    readings: Vec<std::result::Result<PostedReading, BadReading>>,
    today: NaiveDate,
    now: DateTime<Utc>,
    log: &mut PostedLog,
) -> Result<Answer> {
    let mut answer = Answer::default();
    let mut usable = Vec::new();
    for reading in readings {
        match reading {
            Ok(reading) => usable.push(reading),
            Err(bad) => reject(&mut answer, log, bad.index, bad.sensor, bad.problem),
        }
    }
    in_time_order(&mut usable);
    let mapped = setup_queries::sensors(pool).await?;
    let mut sources: HashMap<i64, i64> = HashMap::new();
    for reading in usable {
        let sensor = match sensor_named(&mapped, &reading.sensor) {
            Ok(sensor) => sensor,
            Err(problem) => {
                reject(
                    &mut answer,
                    log,
                    reading.index,
                    Some(reading.sensor),
                    problem,
                );
                continue;
            }
        };
        let source_id = match sources.get(&sensor.id) {
            Some(id) => *id,
            None => {
                let id = source_for(pool, sensor, today).await?;
                sources.insert(sensor.id, id);
                id
            }
        };
        let listed = Listed {
            index: reading.index,
            sensor: sensor.name.clone(),
            observed_at: reading.observed_at,
        };
        match take(pool, sensor, source_id, lookup(&reading), now).await? {
            Taken::Stored | Taken::Starting => {
                if log.stored(&sensor.name) {
                    tracing::info!(sensor = %sensor.name, "Posted sensor readings received");
                }
                answer.stored.push(listed);
            }
            Taken::Unchanged => answer.unchanged.push(listed),
            Taken::Refused(problem) => reject(
                &mut answer,
                log,
                reading.index,
                Some(sensor.name.clone()),
                problem,
            ),
        }
    }
    answer.stored.sort_by_key(|taken| taken.index);
    answer.unchanged.sort_by_key(|taken| taken.index);
    answer.rejected.sort_by_key(|rejected| rejected.index);
    Ok(answer)
}

/// The one mapped sensor that takes posted readings under `name`, or why there is none.
fn sensor_named<'a>(mapped: &'a [Sensor], name: &str) -> std::result::Result<&'a Sensor, String> {
    let named: Vec<&Sensor> = mapped.iter().filter(|s| same_name(&s.name, name)).collect();
    match named.as_slice() {
        [] => Err(format!("No mapped sensor is called {name}.")),
        [sensor] if sensor.way_in == WayIn::HomeAssistant => Err(format!(
            "{} takes its readings from Home Assistant, not from posted readings.",
            sensor.name
        )),
        [sensor] => Ok(sensor),
        _ => Err(format!(
            "More than one sensor is called {name}; rename one in TurfOps."
        )),
    }
}

/// A posted reading in the shape a Home Assistant answer has, so both are judged alike.
fn lookup(reading: &PostedReading) -> Lookup {
    Lookup::Reading(Entity {
        entity_id: String::new(),
        friendly_name: reading.sensor.clone(),
        value: reading.value,
        unit: Some(reading.unit.clone()),
        device_class: None,
        state_class: None,
        reported_at: reading.observed_at,
    })
}

fn reject(
    answer: &mut Answer,
    log: &mut PostedLog,
    index: usize,
    sensor: Option<String>,
    problem: String,
) {
    if log.rejected(sensor.as_deref().unwrap_or_default()) {
        tracing::warn!(sensor = ?sensor, %problem, "Posted sensor reading rejected");
    }
    answer.rejected.push(Rejected {
        index,
        sensor,
        problem,
    });
}
