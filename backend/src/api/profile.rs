use crate::db::queries;
use crate::error::TurfOpsError;
use crate::models::{GrassType, IrrigationType, LawnLocation, LawnProfile, SoilType};
use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use std::str::FromStr;

#[cfg(all(test, feature = "db-tests"))]
mod db_tests;
#[cfg(test)]
mod route_tests;

// @spec APP-PROF-008
pub async fn get_profile(State(state): State<AppState>) -> Result<Json<LawnProfile>, TurfOpsError> {
    let profile = queries::get_default_lawn_profile(&state.pool)
        .await?
        .ok_or_else(|| TurfOpsError::NotFound("No lawn profile found".into()))?;

    Ok(Json(profile))
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub name: Option<String>,
    pub grass_type: Option<String>,
    pub usda_zone: Option<String>,
    pub soil_type: Option<String>,
    pub lawn_size_sqft: Option<f64>,
    pub irrigation_type: Option<String>,
    pub location: Option<LocationRequest>,
}

/// The lawn's location as a request gives it. A timezone is never taken from a request.
#[derive(Debug, Deserialize)]
pub struct LocationRequest {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub place_name: Option<String>,
}

const NEEDS_BOTH: &str = "A location needs both a latitude and a longitude.";
const OUTSIDE: &str = "That location is outside the contiguous United States, the only area \
                       TurfOps has weather data for.";

/// The location a request asks for: a new point, or the current one with a new place name.
// @spec APP-PROF-013, APP-PROF-014, APP-PROF-017
fn requested_location(
    request: LocationRequest,
    current: Option<&LawnLocation>,
) -> Result<LawnLocation, TurfOpsError> {
    let invalid = |message: &str| TurfOpsError::InvalidData(message.into());
    match (request.latitude, request.longitude, current) {
        (Some(latitude), Some(longitude), _) => {
            let place_name = request
                .place_name
                .or_else(|| current.and_then(|c| c.place_name.clone()));
            LawnLocation::at(latitude, longitude, place_name).map_err(|_| invalid(OUTSIDE))
        }
        (None, None, Some(current)) => {
            LawnLocation::at(current.latitude, current.longitude, request.place_name)
                .map_err(|_| invalid(OUTSIDE))
        }
        _ => Err(invalid(NEEDS_BOTH)),
    }
}

/// Update the lawn profile, or create it when none exists yet (filling omitted fields
/// the way startup seeding does), so saving Settings always works.
// @spec APP-PROF-009, APP-PROF-010, APP-PROF-011, APP-PROF-012
pub async fn update_profile(
    State(state): State<AppState>,
    Json(req): Json<UpdateProfileRequest>,
) -> Result<Json<LawnProfile>, TurfOpsError> {
    let existing = queries::get_default_lawn_profile(&state.pool).await?;
    let creating = existing.is_none();
    let mut profile = existing.unwrap_or_else(|| {
        LawnProfile::new("Main Lawn".into(), GrassType::TallFescue, "7a".into())
    });

    if let Some(name) = req.name {
        profile.name = name;
    }
    if let Some(gt) = req.grass_type {
        profile.grass_type = GrassType::from_str(&gt)
            .map_err(|_| TurfOpsError::InvalidData(format!("Unknown grass type: {}", gt)))?;
    }
    if let Some(zone) = req.usda_zone {
        profile.usda_zone = zone;
    }
    // Optional fields: absent = leave alone, empty (or a non-positive size) = clear.
    match req.soil_type.as_deref() {
        None => {}
        Some("") => profile.soil_type = None,
        Some(st) => {
            profile.soil_type =
                Some(SoilType::from_str(st).map_err(|_| {
                    TurfOpsError::InvalidData(format!("Unknown soil type: {}", st))
                })?);
        }
    }
    if let Some(sqft) = req.lawn_size_sqft {
        profile.lawn_size_sqft = Some(sqft).filter(|s| *s > 0.0);
    }
    match req.irrigation_type.as_deref() {
        None => {}
        Some("") => profile.irrigation_type = None,
        Some(it) => {
            profile.irrigation_type = Some(IrrigationType::from_str(it).map_err(|_| {
                TurfOpsError::InvalidData(format!("Unknown irrigation type: {}", it))
            })?);
        }
    }

    if let Some(location) = req.location {
        profile.location = Some(requested_location(location, profile.location.as_ref())?);
    }

    if creating {
        queries::create_lawn_profile(&state.pool, &profile).await?;
    } else {
        queries::update_lawn_profile(&state.pool, &profile).await?;
    }

    state.follow_location().await?;

    // Re-fetch to get the stored timestamps (and id, when created)
    let updated = queries::get_default_lawn_profile(&state.pool)
        .await?
        .ok_or_else(|| TurfOpsError::NotFound("Profile disappeared after update".into()))?;

    Ok(Json(updated))
}
