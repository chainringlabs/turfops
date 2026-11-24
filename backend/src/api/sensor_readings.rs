//! `POST /api/v1/sensor-readings` (WX-SRC-POST): readings sent by the owner's hubs and
//! scripts, mounted only while `SENSOR_TOKEN` is set and guarded by it.

use crate::error::{Result, TurfOpsError};
use crate::logic::weather_sync::posted::{self, BadBody};
use crate::state::AppState;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use take::take_all;

mod take;

#[cfg(test)]
#[path = "sensor_readings/config_tests.rs"]
mod config_tests;
#[cfg(all(test, feature = "db-tests"))]
#[path = "sensor_readings/db_tests.rs"]
mod db_tests;

/// The posted-readings route. `token` unset = the route is not mounted.
#[derive(Clone, Default, Deserialize)]
pub struct SensorReadingsConfig {
    pub token: Option<String>,
}

// @spec DEPLOY-CONFIG-010
impl std::fmt::Debug for SensorReadingsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SensorReadingsConfig")
            .field("token", &self.token.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

/// `SENSOR_TOKEN`: empty means not set; set but shorter than 32 characters stops startup.
// @spec DEPLOY-CONFIG-003, DEPLOY-CONFIG-014
pub fn parse_token(raw: Option<String>) -> Result<Option<String>> {
    crate::mcp::bearer_token("SENSOR_TOKEN", raw)
}

/// `router` with the posted-readings route added when `token` is set.
// @spec WX-SRC-POST-001, WX-SRC-POST-002, WX-SRC-POST-003
pub fn mount(router: Router<AppState>, token: Option<&str>) -> Router<AppState> {
    let Some(token) = token else {
        return router;
    };
    let guarded = Router::new()
        .route("/api/v1/sensor-readings", post(post_readings))
        .route_layer(axum::middleware::from_fn_with_state(
            Arc::<str>::from(token),
            require_token,
        ));
    router.merge(guarded)
}

/// Refuses a request without `Authorization: Bearer <SENSOR_TOKEN>`, storing nothing.
async fn require_token(
    State(token): State<Arc<str>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if crate::mcp::authorized(request.headers(), &token) {
        return next.run(request).await;
    }
    let body = serde_json::json!({
        "error": "Posting sensor readings needs the token set as SENSOR_TOKEN, sent as \
                  \"Authorization: Bearer <token>\"."
    });
    (
        StatusCode::UNAUTHORIZED,
        [(
            header::WWW_AUTHENTICATE,
            r#"Bearer realm="turfops-sensors""#,
        )],
        Json(body),
    )
        .into_response()
}

/// A reading that was stored or unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Taken {
    pub index: usize,
    pub sensor: String,
    pub observed_at: DateTime<Utc>,
}

/// A reading that was refused, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rejected {
    pub index: usize,
    pub sensor: Option<String>,
    pub problem: String,
}

/// What became of each reading in a request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    pub stored: Vec<Taken>,
    pub unchanged: Vec<Taken>,
    pub rejected: Vec<Rejected>,
}

/// Takes the readings in a request.
// @spec WX-SRC-POST-001, WX-SRC-POST-010, WX-SRC-POST-014, WX-SRC-POST-015
pub async fn post_readings(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let Some(lock) = state.sensor_readings.clone() else {
        return TurfOpsError::NotFound("There is no such API endpoint.".into()).into_response();
    };
    let (today, now) = (state.clock.today(), state.clock.now());
    let taken = match posted::readings(&body, now) {
        Ok(taken) => taken,
        Err(BadBody(message)) => return TurfOpsError::InvalidData(message).into_response(),
    };
    // One request at a time: two for one rain counter would measure from one previous reading.
    let mut log = lock.lock().await;
    match take_all(&state.pool, taken, today, now, &mut log).await {
        Ok(answer) if answer.stored.is_empty() && answer.unchanged.is_empty() => {
            (StatusCode::BAD_REQUEST, Json(answer)).into_response()
        }
        Ok(answer) => Json(answer).into_response(),
        Err(error) => error.into_response(),
    }
}
