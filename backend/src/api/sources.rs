//! The lawn's weather sources as the owner sees and chooses them (SETUP).

pub mod view;

#[cfg(all(test, feature = "db-tests"))]
mod db_tests;

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

const OUTSIDE: &str = "TurfOps has no weather data for that location yet. It covers the \
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
    pub inputs: Vec<InputPlan>,
    /// The nearest station that may supply soil temperature, however far.
    pub nearest_soil_station: Option<StationChoice>,
}

/// The stations in the catalog, loading both networks' lists first when it is empty:
/// setup needs them before the lawn has a location for the schedule to act on.
// @spec SETUP-FAIL-004
async fn stations(state: &AppState) -> Result<Vec<(i64, Station)>, TurfOpsError> {
    let held = catalog::stations(&state.pool).await?;
    let Some(fetcher) = state.weather.as_ref().filter(|_| held.is_empty()) else {
        return Ok(held);
    };
    for network in [Network::Uscrn, Network::Scan] {
        fetcher.refresh_catalog(network).await?;
    }
    catalog::stations(&state.pool).await
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
    // The point's elevation is not known until its weather has been fetched once.
    let lawn = Lawn {
        latitude: location.latitude,
        longitude: location.longitude,
        elevation_ft: None,
    };
    let planned = plan::build(Some(&lawn), &catalog, &[], &pins);
    let nearest_soil_station = plan::nearby(&lawn, &catalog, f64::INFINITY)
        .into_iter()
        .find(|near| plan::reports(&near.inputs, Input::SoilTemp10))
        .map(|near| view::station_choice(&near, &catalog));
    Ok(Json(Preview {
        latitude: location.latitude,
        longitude: location.longitude,
        timezone: location.timezone,
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
                elevation_ft: None,
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
