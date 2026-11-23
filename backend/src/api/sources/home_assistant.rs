//! Connecting Home Assistant: the stored connection, testing one, and the sensor
//! entities it offers for mapping.

use super::write::replanned;
use crate::datasources::homeassistant::connection::{Answered, Entity, HaConnection, Unusable};
use crate::db::setup_queries::{self, Credential, Service};
use crate::error::TurfOpsError;
use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

fn refused(why: Unusable) -> TurfOpsError {
    TurfOpsError::InvalidData(why.sentence().into())
}

const NOT_CONNECTED: &str = "Home Assistant isn't connected yet. Enter its address and a token.";
const NEEDS_TOKEN: &str = "Enter a long-lived access token from your Home Assistant profile.";

/// Whether a connection is stored, and where to. Never the token.
// @spec SETUP-CRED-001
#[derive(Debug, Serialize)]
pub struct ConnectionView {
    pub connected: bool,
    pub address: Option<String>,
}

async fn connection_view(state: &AppState) -> Result<Json<ConnectionView>, TurfOpsError> {
    let held = setup_queries::credential(&state.pool, Service::HomeAssistant).await?;
    Ok(Json(ConnectionView {
        connected: held.is_some(),
        address: held.and_then(|held| held.address),
    }))
}

// @spec SETUP-API-007, SETUP-HA-002
pub async fn get_connection(
    State(state): State<AppState>,
) -> Result<Json<ConnectionView>, TurfOpsError> {
    connection_view(&state).await
}

#[derive(Debug, Deserialize)]
pub struct ConnectionRequest {
    pub address: String,
    pub token: String,
}

/// Stores the address and token. It does not have to answer now to be saved.
// @spec SETUP-API-007, SETUP-HA-003, SETUP-API-010
pub async fn put_connection(
    State(state): State<AppState>,
    Json(request): Json<ConnectionRequest>,
) -> Result<Json<ConnectionView>, TurfOpsError> {
    if request.token.trim().is_empty() {
        return Err(TurfOpsError::InvalidData(NEEDS_TOKEN.into()));
    }
    HaConnection::new(&request.address, &request.token).map_err(refused)?;
    let held = Credential {
        address: Some(request.address.trim().trim_end_matches('/').to_string()),
        secret: request.token.trim().to_string(),
    };
    setup_queries::store_credential(&state.pool, Service::HomeAssistant, &held).await?;
    replanned(&state).await?;
    connection_view(&state).await
}

/// Forgets the connection: its address and token are deleted.
// @spec SETUP-API-007, SETUP-CRED-003
pub async fn delete_connection(
    State(state): State<AppState>,
) -> Result<Json<ConnectionView>, TurfOpsError> {
    setup_queries::forget_credential(&state.pool, Service::HomeAssistant).await?;
    replanned(&state).await?;
    connection_view(&state).await
}

/// A connection to try: the one given, or the one stored.
#[derive(Debug, Deserialize, Default)]
pub struct TestRequest {
    pub address: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TestResult {
    #[serde(flatten)]
    pub answered: Answered,
    /// How many sensor entities with a numeric state it has.
    pub numeric_sensors: usize,
}

async fn connection(state: &AppState, given: TestRequest) -> Result<HaConnection, TurfOpsError> {
    let (address, token) = match (given.address, given.token) {
        (Some(address), Some(token)) => (address, token),
        _ => {
            let held = setup_queries::credential(&state.pool, Service::HomeAssistant)
                .await?
                .ok_or_else(|| TurfOpsError::InvalidData(NOT_CONNECTED.into()))?;
            (held.address.unwrap_or_default(), held.secret)
        }
    };
    HaConnection::new(&address, &token).map_err(refused)
}

/// Tries a connection and saves nothing.
// @spec SETUP-API-007, SETUP-HA-001, SETUP-FAIL-002
pub async fn test_connection(
    State(state): State<AppState>,
    Json(request): Json<TestRequest>,
) -> Result<Json<TestResult>, TurfOpsError> {
    let connection = connection(&state, request).await?;
    let answered = connection.probe().await.map_err(refused)?;
    let numeric_sensors = connection.entities().await.map_err(refused)?.len();
    Ok(Json(TestResult {
        answered,
        numeric_sensors,
    }))
}

/// What an entity looks like it measures. The owner confirms, and for soil picks a depth.
// @spec SETUP-MAP-002
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Suggestion {
    Humidity,
    SoilMoisture,
    RainCounter,
    RainAmount,
}

pub fn suggest(entity: &Entity) -> Option<Suggestion> {
    match entity.device_class.as_deref()? {
        "humidity" => Some(Suggestion::Humidity),
        "moisture" => Some(Suggestion::SoilMoisture),
        "precipitation" if entity.state_class.as_deref() == Some("total_increasing") => {
            Some(Suggestion::RainCounter)
        }
        "precipitation" => Some(Suggestion::RainAmount),
        // A thermometer may be in the air or in the soil at any depth.
        _ => None,
    }
}

/// Device classes that can be a weather input, listed first.
const WEATHER_CLASSES: [&str; 4] = ["temperature", "humidity", "moisture", "precipitation"];

#[derive(Debug, Serialize)]
pub struct EntityView {
    pub entity_id: String,
    pub friendly_name: String,
    pub value: f64,
    pub unit: Option<String>,
    pub device_class: Option<String>,
    pub reported_at: DateTime<Utc>,
    pub suggestion: Option<Suggestion>,
    /// The input this entity is already mapped to, as named in requests.
    pub mapped_to: Option<&'static str>,
}

/// The numeric sensor entities, those that look like weather first, then by name.
// @spec SETUP-API-008, SETUP-MAP-001
pub async fn get_entities(
    State(state): State<AppState>,
) -> Result<Json<Vec<EntityView>>, TurfOpsError> {
    let connection = connection(&state, TestRequest::default()).await?;
    let mut entities = connection.entities().await.map_err(refused)?;
    let weather = |entity: &Entity| {
        let class = entity.device_class.as_deref().unwrap_or_default();
        WEATHER_CLASSES.contains(&class)
    };
    entities.sort_by(|a, b| {
        (!weather(a), a.friendly_name.to_lowercase())
            .cmp(&(!weather(b), b.friendly_name.to_lowercase()))
    });
    let mapped = setup_queries::sensors(&state.pool).await?;
    Ok(Json(
        entities
            .into_iter()
            .map(|entity| EntityView {
                suggestion: suggest(&entity),
                mapped_to: mapped
                    .iter()
                    .find(|sensor| sensor.entity_id.as_deref() == Some(&entity.entity_id))
                    .map(|sensor| sensor.input.as_str()),
                entity_id: entity.entity_id,
                friendly_name: entity.friendly_name,
                value: entity.value,
                unit: entity.unit,
                device_class: entity.device_class,
                reported_at: entity.reported_at,
            })
            .collect(),
    ))
}
