//! What the setup flow asks before anything is saved (SETUP).

use crate::datasources::open_meteo::Place;
use crate::error::{Source, TurfOpsError};
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
