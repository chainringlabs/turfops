//! The check shown before a sensor is mapped: its reading, converted, beside the value
//! the lawn would otherwise use, and whatever stands in the way of saving.

use super::read::InUse;
use super::sensors::invalid;
use super::write::input_named;
use crate::clock::Moment;
use crate::datasources::homeassistant::connection::{HaConnection, Lookup, Unusable};
use crate::datasources::sensor_units::{accepted_units, convert, mappable, plausible};
use crate::db::setup_queries::{self, Service};
use crate::error::TurfOpsError;
use crate::logic::weather_record::record::{Asked, WeatherRecord};
use crate::logic::weather_sync::AGE_LIMITS;
use crate::models::weather_record::{Input, SourceKind};
use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CheckRequest {
    pub entity_id: String,
    pub input: String,
}

/// A sensor beside the value it would replace.
// @spec SETUP-MAP-004, SETUP-MAP-005, SETUP-MAP-006
#[derive(Debug, Serialize)]
pub struct Checked {
    /// The sensor's reading in the input's unit, when its unit is one the input accepts.
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub reported_at: Option<DateTime<Utc>>,
    /// What the lawn uses for this input now.
    pub otherwise: Option<InUse>,
    /// Reasons the mapping cannot be saved.
    pub blocking: Vec<String>,
    /// Things to know that do not prevent saving.
    pub warnings: Vec<String>,
}

fn unusable(why: Unusable) -> TurfOpsError {
    invalid(why.sentence())
}

pub(super) async fn check(
    state: &AppState,
    entity_id: &str,
    input: Input,
) -> Result<Checked, TurfOpsError> {
    let held = setup_queries::credential(&state.pool, Service::HomeAssistant)
        .await?
        .ok_or_else(|| {
            invalid("Home Assistant isn't connected yet. Enter its address and a token.")
        })?;
    let connection = HaConnection::new(held.address.as_deref().unwrap_or_default(), &held.secret)
        .map_err(unusable)?;
    let now = state.clock.now();
    let mut checked = Checked {
        value: None,
        unit: None,
        reported_at: None,
        otherwise: otherwise(state, input, now).await?,
        blocking: Vec::new(),
        warnings: Vec::new(),
    };
    let label = input.label().to_lowercase();
    match connection.entity(entity_id).await.map_err(unusable)? {
        Lookup::Missing => checked
            .blocking
            .push(format!("{entity_id} was not found in Home Assistant.")),
        Lookup::NotANumber => checked
            .blocking
            .push(format!("{entity_id} is not reporting a number right now.")),
        Lookup::Reading(entity) => {
            checked.unit = entity.unit.clone();
            checked.reported_at = Some(entity.reported_at);
            checked.value = convert(input, entity.value, entity.unit.as_deref());
            match checked.value {
                None => checked.blocking.push(format!(
                    "This sensor reports in {}. For {label} TurfOps accepts {}.",
                    entity.unit.as_deref().unwrap_or("no unit"),
                    accepted_units(input)
                )),
                Some(value) if !plausible(input, value) => checked.warnings.push(format!(
                    "{value:.1} is outside what {label} can be. Check that this is the right sensor."
                )),
                Some(_) => {}
            }
            let limit = AGE_LIMITS.of(SourceKind::Sensor, input);
            if limit.is_some_and(|limit| now - entity.reported_at > limit) {
                checked.blocking.push(format!(
                    "This sensor last reported {}. For {label} a reading must be newer than that \
                     to be used.",
                    entity.reported_at.format("%b %-d at %H:%M UTC")
                ));
            }
        }
    }
    Ok(checked)
}

/// The value the lawn has for an input now, whatever it rests on.
async fn otherwise(
    state: &AppState,
    input: Input,
    now: DateTime<Utc>,
) -> Result<Option<InUse>, TurfOpsError> {
    let zone = crate::db::queries::get_default_lawn_profile(&state.pool)
        .await?
        .and_then(|profile| profile.location)
        .and_then(|location| location.timezone.parse::<Tz>().ok());
    let Some(zone) = zone else {
        return Ok(None);
    };
    let at = Moment {
        now,
        today: now.with_timezone(&zone).date_naive(),
    };
    let live = WeatherRecord::new(state.pool.clone())
        .fetch_latest(Asked {
            at,
            limits: AGE_LIMITS,
        })
        .await?;
    Ok(live
        .as_ref()
        .and_then(|live| live.get(input))
        .map(|held| InUse {
            value: held.value,
            observed_at: held.observed_at,
            basis: held.basis.clone(),
        }))
}

/// The comparison shown before a mapping is saved. Saves nothing.
// @spec SETUP-API-009, SETUP-MAP-004
pub async fn check_sensor(
    State(state): State<AppState>,
    Json(request): Json<CheckRequest>,
) -> Result<Json<Checked>, TurfOpsError> {
    let input = input_named(&request.input)?;
    if !mappable(input) {
        return Err(invalid(format!(
            "A sensor cannot be mapped to {}.",
            input.label().to_lowercase()
        )));
    }
    Ok(Json(check(&state, request.entity_id.trim(), input).await?))
}
