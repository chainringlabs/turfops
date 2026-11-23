//! The owner's choices about sources: a pinned station, the forecast provider. Each
//! write has the plan rebuilt and the schedule woken before it answers.

use super::read::{sources_view, SourcesView};
use super::view;
use crate::db::setup_queries::{self, Credential, Service};
use crate::db::weather_queries::catalog;
use crate::error::TurfOpsError;
use crate::logic::weather_record::plan::{self, Lawn, PIN_RANGE_MILES};
use crate::logic::weather_sync::planning;
use crate::models::forecast::ForecastProvider;
use crate::models::weather_record::{Input, Station};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

fn invalid(message: impl Into<String>) -> TurfOpsError {
    TurfOpsError::InvalidData(message.into())
}

pub fn input_named(name: &str) -> Result<Input, TurfOpsError> {
    Input::parse(name).ok_or_else(|| invalid(format!("{name} is not an input.")))
}

/// Checks that a station may be pinned for an input from where the lawn is.
// @spec SETUP-API-005, SETUP-PLAN-002
pub fn pinnable(
    input: Input,
    station_ref: i64,
    lawn: &Lawn,
    catalog: &[(i64, Station)],
) -> Result<(), TurfOpsError> {
    let near = plan::nearby(lawn, catalog, f64::INFINITY)
        .into_iter()
        .find(|near| near.station_ref == station_ref)
        .ok_or_else(|| invalid("That station is not in the station list."))?;
    if !plan::reports(&near.inputs, input) {
        return Err(invalid(format!(
            "{} does not report {}.",
            near.name,
            input.label().to_lowercase()
        )));
    }
    if near.distance_miles > PIN_RANGE_MILES {
        return Err(invalid(format!(
            "{} is {:.0} miles away. A pinned station must be within {PIN_RANGE_MILES:.0} miles.",
            near.name, near.distance_miles
        )));
    }
    Ok(())
}

/// Brings the plan in line with what was just saved, starts a fetch pass, and returns
/// the sources as they now stand.
// @spec SETUP-API-010, SYNC-SCHED-008
pub async fn after_write(state: &AppState) -> Result<Json<SourcesView>, TurfOpsError> {
    replanned(state).await?;
    Ok(Json(sources_view(state).await?))
}

/// Brings the plan in line with what was just saved and starts a fetch pass.
// @spec SETUP-API-010, SYNC-SCHED-008
pub async fn replanned(state: &AppState) -> Result<(), TurfOpsError> {
    planning::rebuild(&state.pool).await?;
    if let Some(waker) = &state.weather_waker {
        waker.wake();
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct PinRequest {
    pub station_ref: i64,
}

// @spec SETUP-API-005
pub async fn put_pin(
    State(state): State<AppState>,
    Path(input): Path<String>,
    Json(request): Json<PinRequest>,
) -> Result<Json<SourcesView>, TurfOpsError> {
    let input = input_named(&input)?;
    let lawn = view::lawn(&state.pool).await?;
    let stations = catalog::stations(&state.pool).await?;
    pinnable(input, request.station_ref, &lawn, &stations)?;
    setup_queries::pin(&state.pool, input, request.station_ref).await?;
    after_write(&state).await
}

// @spec SETUP-API-005
pub async fn delete_pin(
    State(state): State<AppState>,
    Path(input): Path<String>,
) -> Result<Json<SourcesView>, TurfOpsError> {
    setup_queries::unpin(&state.pool, input_named(&input)?).await?;
    after_write(&state).await
}

#[derive(Debug, Deserialize)]
pub struct ForecastProviderRequest {
    pub provider: ForecastProvider,
    /// A key for the provider, when it needs one. Stored, never returned.
    pub key: Option<String>,
}

impl ForecastProviderRequest {
    /// The key the request carries, when it carries one worth storing.
    pub fn new_key(&self) -> Option<String> {
        self.key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(str::to_string)
    }
}

const NEEDS_KEY: &str = "OpenWeatherMap needs a key. Enter one, or choose another provider.";

/// Checks that the chosen provider can be used: OpenWeatherMap needs a key, given now
/// or stored already.
// @spec SETUP-PLAN-004, WX-SRC-CFG-003
pub async fn usable(
    state: &AppState,
    request: &ForecastProviderRequest,
) -> Result<(), TurfOpsError> {
    if request.provider != ForecastProvider::OpenWeatherMap || request.new_key().is_some() {
        return Ok(());
    }
    let stored = setup_queries::credential(&state.pool, Service::OpenWeatherMap).await?;
    stored.map(|_| ()).ok_or_else(|| invalid(NEEDS_KEY))
}

// @spec SETUP-API-006, SETUP-CRED-001
pub async fn put_forecast_provider(
    State(state): State<AppState>,
    Json(request): Json<ForecastProviderRequest>,
) -> Result<Json<SourcesView>, TurfOpsError> {
    usable(&state, &request).await?;
    if let Some(secret) = request.new_key() {
        let held = Credential {
            address: None,
            secret,
        };
        setup_queries::store_credential(&state.pool, Service::OpenWeatherMap, &held).await?;
    }
    setup_queries::set_forecast_provider(&state.pool, request.provider).await?;
    after_write(&state).await
}
