//! `GET /api/v1/sources`: the lawn's inputs and what stands behind each, the sources in
//! use and how they are doing, the forecast provider and the stored connections.

use super::view::{self, InputPlan, Planned};
use crate::clock::Moment;
use crate::db::setup_queries::{self, Service};
use crate::db::soil_estimate_queries;
use crate::db::weather_queries::plan;
use crate::error::TurfOpsError;
use crate::logic::soil_estimate::fit::StoredFit;
use crate::logic::soil_estimate::scores::{self, Calibration, CrossingAccuracy};
use crate::logic::weather_record::live::LiveReading;
use crate::logic::weather_record::record::{Asked, WeatherRecord};
use crate::logic::weather_sync::forecast::ForecastStatus;
use crate::logic::weather_sync::AGE_LIMITS;
use crate::models::forecast::ForecastProvider;
use crate::models::weather_basis::Basis;
use crate::models::weather_record::{Input, SourceKind};
use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use serde::Serialize;

/// Where the lawn is.
#[derive(Debug, Serialize)]
pub struct LocationView {
    pub place_name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
}

/// The value an input has now.
#[derive(Debug, Serialize)]
pub struct InUse {
    pub value: f64,
    pub observed_at: DateTime<Utc>,
    pub basis: Basis,
}

/// How far off the soil estimate typically is, from its published scores.
// @spec SETUP-PAGE-002, WX-EST-SCORE-003
#[derive(Debug, Serialize)]
pub struct EstimateAccuracy {
    /// Root-mean-square daily error, °F.
    pub typical_error_f: f64,
    pub fitted_on: Calibration,
    /// Each crossing scored at the depth under that calibration, and whether the
    /// estimate may date it.
    pub crossings: Vec<CrossingAccuracy>,
}

#[derive(Debug, Serialize)]
pub struct InputView {
    #[serde(flatten)]
    pub plan: InputPlan,
    pub in_use: Option<InUse>,
    /// For an input the soil estimate stands behind, once it has been fitted.
    pub estimate_accuracy: Option<EstimateAccuracy>,
    /// The station the owner pinned for this input.
    pub pinned_station_ref: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SourceView {
    pub id: i64,
    pub name: String,
    pub kind: SourceKind,
    /// The inputs the plan ranks it for, as named in requests.
    pub inputs: Vec<&'static str>,
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub held_from: Option<NaiveDate>,
    pub held_through: Option<NaiveDate>,
    pub wanted_from: Option<NaiveDate>,
    pub attribution: &'static str,
    /// For a Home Assistant sensor whose entity Home Assistant no longer has: the
    /// sensor's id, so the page can offer to unmap or remap it.
    pub missing_sensor: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct Connections {
    /// Whether a Home Assistant address and token are stored. Never the token.
    pub home_assistant: bool,
    /// Whether an OpenWeatherMap key is stored. Never the key.
    pub openweathermap_key: bool,
    /// Whether posted sensor readings are taken: whether `SENSOR_TOKEN` is set.
    pub posted_readings: bool,
}

#[derive(Debug, Serialize)]
pub struct SourcesView {
    pub location: Option<LocationView>,
    pub inputs: Vec<InputView>,
    pub sources: Vec<SourceView>,
    pub forecast_provider: ForecastProvider,
    /// How fetching the forecast is going.
    pub forecast: ForecastStatus,
    pub connections: Connections,
}

/// Who to credit for a source's data.
// @spec SETUP-PAGE-004
fn attribution(kind: SourceKind, key: &str) -> &'static str {
    match kind {
        SourceKind::Sensor => "Your sensor",
        SourceKind::Station if key.contains("Scan") => {
            "USDA NRCS Soil Climate Analysis Network (SCAN)"
        }
        SourceKind::Station => "NOAA U.S. Climate Reference Network (USCRN)",
        SourceKind::Grid if key.contains("nrcc") => {
            "Northeast Regional Climate Center, via RCC-ACIS"
        }
        SourceKind::Grid => "PRISM Climate Group, Oregon State University, via RCC-ACIS",
        SourceKind::Analysis => "Weather data by Open-Meteo.com (CC BY 4.0)",
        SourceKind::Estimate => "TurfOps model, fitted on USCRN soil and PRISM air temperature",
    }
}

fn depth_cm(input: Input) -> Option<i32> {
    match input {
        Input::SoilTemp5 => Some(5),
        Input::SoilTemp10 => Some(10),
        _ => None,
    }
}

// @spec SETUP-API-003
pub async fn get_sources(State(state): State<AppState>) -> Result<Json<SourcesView>, TurfOpsError> {
    Ok(Json(sources_view(&state).await?))
}

/// Every input with its plan, the value it has now, how good its estimate is, and the
/// station pinned for it.
async fn input_views(
    pool: &sqlx::PgPool,
    live: Option<&LiveReading>,
) -> Result<Vec<InputView>, TurfOpsError> {
    let placed = plan::read(pool).await?;
    let pins = setup_queries::pins(pool).await?;
    let fits = soil_estimate_queries::all(pool).await?;
    let plans = view::input_plans(placed.iter().map(Planned::from));
    Ok(plans
        .into_iter()
        .zip(Input::ALL)
        .map(|(plan, input)| {
            let estimated = plan.sources.iter().any(|s| s.kind == SourceKind::Estimate);
            let fit = depth_cm(*input)
                .filter(|_| estimated)
                .and_then(|cm| fits.iter().find(|fit| fit.depth_cm == cm));
            InputView {
                in_use: live.and_then(|l| l.get(*input)).map(|held| InUse {
                    value: held.value,
                    observed_at: held.observed_at,
                    basis: held.basis.clone(),
                }),
                estimate_accuracy: fit.and_then(accuracy_of),
                pinned_station_ref: pins.iter().find(|(pinned, _)| pinned == input).map(|p| p.1),
                plan,
            }
        })
        .collect())
}

/// The published daily error of a fit's depth under what it was fitted on.
// @spec WX-EST-SCORE-003, SETUP-API-014
fn accuracy_of(fit: &StoredFit) -> Option<EstimateAccuracy> {
    let depth = fit.depth_cm as u32;
    Some(EstimateAccuracy {
        typical_error_f: scores::embedded().daily_error(depth, fit.calibration)?,
        fitted_on: fit.calibration,
        crossings: scores::embedded().crossings_at(depth, fit.calibration),
    })
}

/// The view every sources read and write answers with.
// @spec SETUP-API-013
pub async fn sources_view(state: &AppState) -> Result<SourcesView, TurfOpsError> {
    let pool = &state.pool;
    let location = crate::db::queries::get_default_lawn_profile(pool)
        .await?
        .and_then(|profile| profile.location);
    let record = WeatherRecord::new(pool.clone());
    let live = match location
        .as_ref()
        .and_then(|l| l.timezone.parse::<Tz>().ok())
    {
        Some(zone) => {
            let now = state.clock.now();
            let at = Moment {
                now,
                today: now.with_timezone(&zone).date_naive(),
            };
            record
                .fetch_latest(Asked {
                    at,
                    limits: AGE_LIMITS,
                })
                .await?
        }
        None => None,
    };
    let inputs = input_views(pool, live.as_ref()).await?;
    let sensors = setup_queries::sensors(pool).await?;
    let mut sources: Vec<SourceView> = record
        .status()
        .await?
        .into_iter()
        .map(|status| SourceView {
            missing_sensor: missing_sensor(&sensors, &status.key, status.last_error.as_deref()),
            attribution: attribution(status.kind, &status.key),
            id: status.source_id,
            name: status.name,
            kind: status.kind,
            inputs: status.inputs.iter().map(Input::as_str).collect(),
            last_success: status.last_success,
            last_failure: status.last_failure,
            last_error: status.last_error,
            held_from: status.held.map(|held| held.0),
            held_through: status.held.map(|held| held.1),
            wanted_from: status.wanted_from,
        })
        .collect();
    // Failing sources first.
    sources.sort_by_key(|source| source.last_error.is_none());
    Ok(SourcesView {
        location: location.map(|l| LocationView {
            place_name: l.place_name,
            latitude: l.latitude,
            longitude: l.longitude,
            timezone: l.timezone,
        }),
        inputs,
        sources,
        forecast_provider: setup_queries::forecast_provider(pool).await?,
        // @spec SETUP-API-015
        forecast: state.sync_service.read().await.forecast_status().await?,
        connections: Connections {
            home_assistant: setup_queries::credential(pool, Service::HomeAssistant)
                .await?
                .is_some(),
            openweathermap_key: setup_queries::credential(pool, Service::OpenWeatherMap)
                .await?
                .is_some(),
            posted_readings: state.sensor_readings.is_some(),
        },
    })
}

/// The mapped Home Assistant sensor behind the source `key`, when its current problem is
/// that Home Assistant no longer has its entity.
// @spec SETUP-FAIL-003
fn missing_sensor(
    sensors: &[setup_queries::Sensor],
    key: &str,
    error: Option<&str>,
) -> Option<i64> {
    use crate::logic::weather_sync::sensor_reading::Problem;
    let error = error?;
    sensors
        .iter()
        .find(|sensor| {
            sensor.source_key() == key && sensor.way_in == setup_queries::WayIn::HomeAssistant
        })
        .filter(|sensor| {
            sensor
                .entity_id
                .as_deref()
                .is_some_and(|entity| error == Problem::NotFound.sentence(sensor.input, entity))
        })
        .map(|sensor| sensor.id)
}
