//! The lawn's weather sources as the owner sees and chooses them (SETUP).

pub mod home_assistant;
pub mod read;
pub mod sensor_check;
pub mod sensors;
pub mod view;
pub mod write;

#[cfg(all(test, feature = "db-tests"))]
mod db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod elevation_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod home_assistant_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod read_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod sensor_check_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod sensors_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod write_db_tests;

use crate::db::weather_queries::catalog;
use crate::error::TurfOpsError;
use crate::logic::weather_record::plan::{self, Lawn, Pin, PIN_RANGE_MILES};
use crate::models::weather_record::{Input, Network, Station};
use crate::models::LawnLocation;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use view::{InputPlan, StationChoice};

pub(crate) const OUTSIDE: &str =
    "TurfOps has no weather data for that location yet. It covers the \
                       contiguous United States.";

#[derive(Debug, Deserialize)]
pub struct PreviewQuery {
    pub lat: f64,
    pub lon: f64,
    /// Pins to try, as `input:station` pairs separated by commas.
    pub pins: Option<String>,
}

/// What a point would get, before it is saved.
#[derive(Debug, Serialize)]
pub struct Preview {
    /// The point as it would be stored, and the timezone that follows from it.
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
    /// The point's elevation in feet, when it could be had.
    pub elevation_ft: Option<f64>,
    pub inputs: Vec<InputPlan>,
    /// The nearest station that may supply soil temperature, however far.
    pub nearest_soil_station: Option<StationChoice>,
}

/// The stations in the catalog, loading first the list of each network the catalog holds
/// no station of: setup needs them before the lawn has a location for the schedule to act
/// on. A list that cannot be loaded fails the request with that network's outage.
// @spec SETUP-FAIL-004
async fn stations(state: &AppState) -> Result<Vec<(i64, Station)>, TurfOpsError> {
    let held = catalog::stations(&state.pool).await?;
    let Some(fetcher) = state.weather.as_ref() else {
        return Ok(held);
    };
    let missing: Vec<Network> = [Network::Uscrn, Network::Scan]
        .into_iter()
        .filter(|network| !held.iter().any(|(_, s)| s.network == *network))
        .collect();
    if missing.is_empty() {
        return Ok(held);
    }
    for network in missing {
        fetcher.refresh_catalog(network).await?;
    }
    catalog::stations(&state.pool).await
}

/// A point's elevation, asked of the hourly analysis so that the elevation guard applies
/// to a preview as it will to the saved plan. If it cannot be had the preview goes on
/// without the guard, as a new plan does until its first fetch.
// @spec SETUP-API-002, WX-REC-PLAN-007
async fn elevation_of(state: &AppState, at: &LawnLocation) -> Option<f64> {
    let fetcher = state.weather.as_ref()?;
    match fetcher
        .elevation_ft(at.latitude, at.longitude, state.clock.now())
        .await
    {
        Ok(elevation) => elevation,
        Err(error) => {
            tracing::warn!(
                ?error,
                "A point's elevation could not be fetched for a preview"
            );
            None
        }
    }
}

fn parse_pins(text: Option<&str>) -> Result<Vec<Pin>, TurfOpsError> {
    let bad =
        || TurfOpsError::InvalidData("A pin is an input and a station, as input:station.".into());
    text.unwrap_or_default()
        .split(',')
        .filter(|pair| !pair.trim().is_empty())
        .map(|pair| {
            let (input, station) = pair.trim().split_once(':').ok_or_else(bad)?;
            Ok(Pin {
                input: Input::parse(input).ok_or_else(bad)?,
                station_ref: station.parse().map_err(|_| bad())?,
            })
        })
        .collect()
}

/// The basis plan a point would get. Saves nothing.
// @spec SETUP-API-002, SETUP-LOC-003, SETUP-LOC-004, SETUP-PLAN-001, SETUP-PLAN-003
pub async fn preview(
    State(state): State<AppState>,
    Query(query): Query<PreviewQuery>,
) -> Result<Json<Preview>, TurfOpsError> {
    let location = LawnLocation::at(query.lat, query.lon, None)
        .map_err(|_| TurfOpsError::InvalidData(OUTSIDE.into()))?;
    let pins = parse_pins(query.pins.as_deref())?;
    let catalog = stations(&state).await?;
    let lawn = Lawn {
        latitude: location.latitude,
        longitude: location.longitude,
        elevation_ft: elevation_of(&state, &location).await,
    };
    // The owner's sensors speak first wherever they are mapped, here as once saved.
    let sensors = crate::logic::weather_sync::planning::mapped_sensors(&state.pool).await?;
    let planned = plan::build(Some(&lawn), &catalog, &sensors, &pins);
    let nearest_soil_station = plan::nearby(&lawn, &catalog, f64::INFINITY)
        .into_iter()
        .find(|near| plan::reports(&near.inputs, Input::SoilTemp10))
        .map(|near| view::station_choice(&near, &catalog));
    Ok(Json(Preview {
        latitude: location.latitude,
        longitude: location.longitude,
        timezone: location.timezone,
        elevation_ft: lawn.elevation_ft,
        inputs: view::input_plans(planned.iter().map(view::Planned::from)),
        nearest_soil_station,
    }))
}

#[derive(Debug, Deserialize)]
pub struct StationsQuery {
    pub input: String,
    /// A point to measure from, for a location not yet saved; the lawn's otherwise.
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

/// The stations within pinning range that may supply an input, nearest first.
// @spec SETUP-API-004, SETUP-PLAN-002
pub async fn stations_for(
    State(state): State<AppState>,
    Query(query): Query<StationsQuery>,
) -> Result<Json<Vec<StationChoice>>, TurfOpsError> {
    let input = Input::parse(&query.input)
        .ok_or_else(|| TurfOpsError::InvalidData(format!("{} is not an input.", query.input)))?;
    let lawn = match query.lat.zip(query.lon) {
        Some((latitude, longitude)) => {
            let at = LawnLocation::at(latitude, longitude, None)
                .map_err(|_| TurfOpsError::InvalidData(OUTSIDE.into()))?;
            Lawn {
                latitude: at.latitude,
                longitude: at.longitude,
                elevation_ft: elevation_of(&state, &at).await,
            }
        }
        None => view::lawn(&state.pool).await?,
    };
    let catalog = stations(&state).await?;
    Ok(Json(
        plan::nearby(&lawn, &catalog, PIN_RANGE_MILES)
            .iter()
            .filter(|near| plan::reports(&near.inputs, input))
            .map(|near| view::station_choice(near, &catalog))
            .collect(),
    ))
}
