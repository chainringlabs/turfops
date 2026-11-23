//! The owner's sensors: which are mapped, mapping one, changing and removing a mapping,
//! and the check shown before a mapping is saved.

use super::sensor_check::check;
use super::write::{input_named, replanned};
use crate::datasources::sensor_units::mappable;
use crate::db::setup_queries::{self, NewSensor, RainKind, Sensor, WayIn};
use crate::error::TurfOpsError;
use crate::models::weather_record::Input;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

pub(super) fn invalid(message: impl Into<String>) -> TurfOpsError {
    TurfOpsError::InvalidData(message.into())
}

#[derive(Debug, Serialize)]
pub struct SensorView {
    pub id: i64,
    pub name: String,
    /// `HomeAssistant` or `Posted`.
    pub way_in: &'static str,
    pub entity_id: Option<String>,
    /// The input it is mapped to, as named in requests, and as the owner reads it.
    pub input: &'static str,
    pub label: &'static str,
    /// For a rain sensor: `Counter` or `Amount`.
    pub rain_kind: Option<&'static str>,
}

fn view(sensor: Sensor) -> SensorView {
    SensorView {
        id: sensor.id,
        name: sensor.name,
        way_in: match sensor.way_in {
            WayIn::HomeAssistant => "HomeAssistant",
            WayIn::Posted => "Posted",
        },
        entity_id: sensor.entity_id,
        input: sensor.input.as_str(),
        label: sensor.input.label(),
        rain_kind: sensor.rain_kind.map(|kind| match kind {
            RainKind::Counter => "Counter",
            RainKind::Amount => "Amount",
        }),
    }
}

async fn all(state: &AppState) -> Result<Json<Vec<SensorView>>, TurfOpsError> {
    let sensors = setup_queries::sensors(&state.pool).await?;
    Ok(Json(sensors.into_iter().map(view).collect()))
}

// @spec SETUP-API-009
pub async fn get_sensors(
    State(state): State<AppState>,
) -> Result<Json<Vec<SensorView>>, TurfOpsError> {
    all(&state).await
}

/// What a sensor is to be mapped to.
#[derive(Debug, Deserialize)]
pub struct MappingRequest {
    pub name: String,
    /// The Home Assistant entity; absent for a sensor whose readings are posted.
    pub entity_id: Option<String>,
    pub input: String,
    pub rain_kind: Option<String>,
    /// Set to take the input from the sensor that has it now.
    #[serde(default)]
    pub replace: bool,
}

/// The input and rain kind a request names, checked against each other.
// @spec SETUP-MAP-003
fn mapping(
    input: &str,
    rain_kind: Option<&str>,
) -> Result<(Input, Option<RainKind>), TurfOpsError> {
    let input = input_named(input)?;
    if !mappable(input) {
        return Err(invalid(format!(
            "A sensor cannot be mapped to {}.",
            input.label().to_lowercase()
        )));
    }
    let kind = match (input, rain_kind) {
        (Input::Rain, Some("Counter")) => Some(RainKind::Counter),
        (Input::Rain, Some("Amount")) => Some(RainKind::Amount),
        (Input::Rain, _) => {
            return Err(invalid(
                "Say whether the rain sensor is a counter that only rises or an amount since \
                 its last reading.",
            ))
        }
        (_, _) => None,
    };
    Ok((input, kind))
}

/// Refuses an entity that is already mapped to another input.
// @spec SETUP-MAP-008
fn entity_is_free(
    mapped: &[Sensor],
    entity_id: Option<&str>,
    input: Input,
) -> Result<(), TurfOpsError> {
    let taken = entity_id.and_then(|id| {
        mapped
            .iter()
            .find(|sensor| sensor.entity_id.as_deref() == Some(id) && sensor.input != input)
    });
    match taken {
        None => Ok(()),
        Some(taken) => Err(TurfOpsError::Conflict(format!(
            "{} is already mapped to {}. Unmap it first.",
            taken.name,
            taken.input.label().to_lowercase()
        ))),
    }
}

/// Maps a sensor to an input.
// @spec SETUP-API-009, SETUP-MAP-005, SETUP-MAP-007, SETUP-MAP-008, SETUP-MAP-010, SETUP-API-010
pub async fn post_sensor(
    State(state): State<AppState>,
    Json(request): Json<MappingRequest>,
) -> Result<Json<Vec<SensorView>>, TurfOpsError> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err(invalid("Give the sensor a name."));
    }
    let (input, rain_kind) = mapping(&request.input, request.rain_kind.as_deref())?;
    let entity_id = request
        .entity_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let mapped = setup_queries::sensors(&state.pool).await?;
    entity_is_free(&mapped, entity_id, input)?;
    if let Some(id) = entity_id {
        let checked = check(&state, id, input).await?;
        if let Some(problem) = checked.blocking.first() {
            return Err(invalid(problem.clone()));
        }
    }
    match mapped.iter().find(|sensor| sensor.input == input) {
        Some(holder) if !request.replace => {
            return Err(TurfOpsError::Conflict(format!(
                "{} already supplies {}. Replace it with {name}?",
                holder.name,
                input.label().to_lowercase()
            )));
        }
        Some(holder) => {
            setup_queries::unmap_sensor(&state.pool, holder.id).await?;
        }
        None => {}
    }
    let new = NewSensor {
        name: name.to_string(),
        entity_id: entity_id.map(str::to_string),
        input,
        rain_kind,
    };
    setup_queries::map_sensor(&state.pool, &new).await?;
    replanned(&state).await?;
    all(&state).await
}

#[derive(Debug, Deserialize)]
pub struct ChangeRequest {
    pub name: String,
    pub input: String,
    pub rain_kind: Option<String>,
}

/// Renames a sensor or maps it to another input.
// @spec SETUP-API-009, SETUP-API-010
pub async fn put_sensor(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(request): Json<ChangeRequest>,
) -> Result<Json<Vec<SensorView>>, TurfOpsError> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err(invalid("Give the sensor a name."));
    }
    let (input, rain_kind) = mapping(&request.input, request.rain_kind.as_deref())?;
    let mapped = setup_queries::sensors(&state.pool).await?;
    if let Some(holder) = mapped.iter().find(|s| s.input == input && s.id != id) {
        return Err(TurfOpsError::Conflict(format!(
            "{} already supplies {}. Unmap it first.",
            holder.name,
            input.label().to_lowercase()
        )));
    }
    setup_queries::change_sensor(&state.pool, id, name, input, rain_kind)
        .await?
        .ok_or_else(|| TurfOpsError::NotFound("No mapped sensor has that id.".into()))?;
    replanned(&state).await?;
    all(&state).await
}

/// Unmaps a sensor. Its stored readings are kept and its input goes back to the next
/// source in the plan.
// @spec SETUP-API-009, SETUP-MAP-009, SETUP-API-010
pub async fn delete_sensor(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<SensorView>>, TurfOpsError> {
    if !setup_queries::unmap_sensor(&state.pool, id).await? {
        return Err(TurfOpsError::NotFound(
            "No mapped sensor has that id.".into(),
        ));
    }
    replanned(&state).await?;
    all(&state).await
}
