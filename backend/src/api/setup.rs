//! What the setup flow asks before anything is saved (SETUP).

use crate::api::sources::read::SourcesView;
use crate::api::sources::view;
use crate::api::sources::write::{self, ForecastProviderRequest};
use crate::datasources::open_meteo::Place;
use crate::db::setup_queries::{self, Applied};
use crate::db::weather_queries::catalog;
use crate::error::{Source, TurfOpsError};
use crate::logic::weather_record::plan::Lawn;
use crate::models::LawnLocation;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

#[cfg(all(test, feature = "db-tests"))]
pub(crate) mod db_tests;

#[derive(Debug, Deserialize)]
pub struct PlaceQuery {
    pub q: Option<String>,
}

/// Candidate places for a typed name or postal code. The typed text is sent to the
/// place-search service and nothing else is.
// @spec SETUP-API-001, SETUP-FAIL-001
pub async fn places(
    State(state): State<AppState>,
    Query(query): Query<PlaceQuery>,
) -> Result<Json<Vec<Place>>, TurfOpsError> {
    let fetcher = state
        .weather
        .as_ref()
        .ok_or_else(|| TurfOpsError::unavailable(Source::PlaceSearch, "no weather clients"))?;
    let typed = query.q.unwrap_or_default();
    Ok(Json(fetcher.places(&typed).await?))
}

/// Everything the first steps of setup decide, saved together.
#[derive(Debug, Deserialize)]
pub struct ApplyRequest {
    pub location: Option<ApplyLocation>,
    /// The whole set of pins: those not listed are removed.
    pub pins: Option<Vec<ApplyPin>>,
    pub forecast_provider: Option<ForecastProviderRequest>,
}

#[derive(Debug, Deserialize)]
pub struct ApplyLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub place_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApplyPin {
    pub input: String,
    pub station_ref: i64,
}

/// Saves a location, pins and a forecast provider, all or none, then has the plan
/// rebuilt once and the schedule woken once.
// @spec SETUP-API-012, SETUP-PLAN-005
pub async fn apply(
    State(state): State<AppState>,
    Json(request): Json<ApplyRequest>,
) -> Result<Json<SourcesView>, TurfOpsError> {
    let invalid = |message: &str| TurfOpsError::InvalidData(message.into());
    let location = request
        .location
        .map(|at| {
            let place_name = at.place_name.filter(|name| !name.trim().is_empty());
            LawnLocation::at(at.latitude, at.longitude, place_name)
                .map_err(|_| invalid(crate::api::sources::OUTSIDE))
        })
        .transpose()?;
    // Pins are judged from where the lawn will be.
    let pins = match request.pins {
        None => None,
        Some(pins) => {
            let lawn = match &location {
                Some(at) => Lawn {
                    latitude: at.latitude,
                    longitude: at.longitude,
                    elevation_ft: None,
                },
                None => view::lawn(&state.pool).await?,
            };
            let stations = catalog::stations(&state.pool).await?;
            let mut checked = Vec::new();
            for pin in pins {
                let input = write::input_named(&pin.input)?;
                write::pinnable(input, pin.station_ref, &lawn, &stations)?;
                checked.push((input, pin.station_ref));
            }
            Some(checked)
        }
    };
    if let Some(provider) = &request.forecast_provider {
        write::usable(&state, provider).await?;
    }
    let applied = Applied {
        location,
        pins,
        forecast_provider: request.forecast_provider.as_ref().map(|p| p.provider),
        openweathermap_key: request.forecast_provider.as_ref().and_then(|p| p.new_key()),
    };
    if !setup_queries::apply(&state.pool, &applied).await? {
        return Err(TurfOpsError::NotFound("No lawn profile found".into()));
    }
    write::after_write(&state).await
}
