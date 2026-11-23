//! The shapes the sources endpoints answer in.

use crate::db::weather_queries::lawn as lawn_queries;
use crate::db::weather_queries::plan::Placed;
use crate::error::TurfOpsError;
use crate::logic::weather_record::plan::{Lawn, Nearby, Ranked};
use crate::models::weather_record::{Input, SourceKind, Station};
use serde::Serialize;
use sqlx::PgPool;

/// Where an input stands: on a measurement, on an estimate, or on nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Standing {
    Measured,
    Estimated,
    NoSource,
}

/// A source in an input's plan.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlannedSource {
    pub name: String,
    pub kind: SourceKind,
    pub distance_miles: Option<f64>,
    /// The owner chose this station.
    pub pinned: bool,
}

/// An input and the sources that speak for it, in order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InputPlan {
    /// The input's name in requests, such as `soil_temp_10`.
    pub input: &'static str,
    pub label: &'static str,
    pub standing: Standing,
    pub sources: Vec<PlannedSource>,
}

/// One place in a plan, built or stored.
pub struct Planned {
    pub input: Input,
    pub source: PlannedSource,
}

impl From<&Ranked> for Planned {
    fn from(ranked: &Ranked) -> Self {
        Planned {
            input: ranked.input,
            source: PlannedSource {
                name: ranked.source.name.clone(),
                kind: ranked.source.kind,
                distance_miles: ranked.source.distance_miles,
                pinned: ranked.pinned,
            },
        }
    }
}

impl From<&Placed> for Planned {
    fn from(placed: &Placed) -> Self {
        Planned {
            input: placed.input,
            source: PlannedSource {
                name: placed.source.name.clone(),
                kind: placed.source.kind,
                distance_miles: placed.source.distance_miles,
                pinned: placed.pinned,
            },
        }
    }
}

/// Every input with its sources in plan order; an input with none is listed as having
/// no source. `planned` is in rank order within each input.
// @spec SETUP-PLAN-001
pub fn input_plans(planned: impl Iterator<Item = Planned>) -> Vec<InputPlan> {
    let mut plans: Vec<InputPlan> = Input::ALL
        .iter()
        .map(|input| InputPlan {
            input: input.as_str(),
            label: input.label(),
            standing: Standing::NoSource,
            sources: Vec::new(),
        })
        .collect();
    for place in planned {
        let Some(plan) = plans
            .iter_mut()
            .find(|plan| plan.input == place.input.as_str())
        else {
            continue;
        };
        if plan.sources.is_empty() {
            plan.standing = match place.source.kind {
                SourceKind::Sensor | SourceKind::Station => Standing::Measured,
                _ => Standing::Estimated,
            };
        }
        plan.sources.push(place.source);
    }
    plans
}

/// A station the owner may pin.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StationChoice {
    pub station_ref: i64,
    pub network: &'static str,
    pub name: String,
    pub state: String,
    pub distance_miles: f64,
    /// The station's elevation less the lawn's, when both are known.
    pub elevation_difference_ft: Option<f64>,
    pub first_year: Option<i32>,
    pub last_year: Option<i32>,
}

pub fn station_choice(near: &Nearby, catalog: &[(i64, Station)]) -> StationChoice {
    let listed = catalog.iter().find(|(id, _)| *id == near.station_ref);
    StationChoice {
        station_ref: near.station_ref,
        network: match near.network {
            crate::models::weather_record::Network::Uscrn => "USCRN",
            crate::models::weather_record::Network::Scan => "SCAN",
        },
        name: near.name.clone(),
        state: near.state.clone(),
        distance_miles: near.distance_miles,
        elevation_difference_ft: near.elevation_difference_ft,
        first_year: listed.and_then(|(_, station)| station.first_year),
        last_year: listed.and_then(|(_, station)| station.last_year),
    }
}

/// The lawn's point and elevation, or the location-not-set outage.
pub async fn lawn(pool: &PgPool) -> Result<Lawn, TurfOpsError> {
    let location = crate::db::queries::get_default_lawn_profile(pool)
        .await?
        .and_then(|profile| profile.location)
        .ok_or_else(TurfOpsError::location_not_set)?;
    Ok(Lawn {
        latitude: location.latitude,
        longitude: location.longitude,
        elevation_ft: lawn_queries::elevation_ft(pool).await?,
    })
}
