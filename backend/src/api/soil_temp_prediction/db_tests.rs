//! `GET /api/v1/soil-temp-forecast` from the weather record (`--features db-tests`).

use super::*;
use crate::clock::Clock;
use crate::logic::data_sync::test_support::{forecast, forecast_day};
use crate::logic::data_sync::DataSyncService;
use crate::logic::weather_record::lab::{d, Lab, PRISM};
use crate::logic::weather_sync::forecast::ForecastSlot;
use crate::models::soil_temp_prediction::OutlookMethod;
use crate::models::weather_basis::BasisKind;
use axum::response::IntoResponse;
use chrono::{TimeZone, Utc};
use sqlx::PgPool;

/// Noon Eastern on July 20, 2026.
fn clock() -> Clock {
    Clock::Frozen {
        at: Utc.with_ymd_and_hms(2026, 7, 20, 16, 0, 0).unwrap(),
        zone: chrono_tz::America::New_York,
    }
}

/// Ten full forecast days from July 20, warming 1°F a day.
fn ten_days() -> ForecastSlot {
    ForecastSlot::holding(forecast(
        (0..10)
            .map(|n| forecast_day(d(20 + n), 24, 82.0 + n as f64, 64.0 + n as f64))
            .collect(),
    ))
}

async fn state(pool: PgPool, slot: ForecastSlot) -> AppState {
    let mut config = crate::logic::data_sync::test_env::config(&[]);
    config.clock = clock();
    let sync = DataSyncService::initialize(&config, pool.clone())
        .await
        .with_forecast(slot);
    AppState::new(pool, sync, None, false, clock())
}

/// Measured 10 cm soil from the station and PRISM air on July 1–19, soil 2°F above the
/// day's mean air, which swings so the fit has something to explain.
async fn measured_july(lab: &Lab) {
    for day in 1..=19 {
        let air = 70.0 + f64::from(day % 5) * 2.0;
        lab.grid(PRISM, day, air + 8.0, air - 8.0).await;
        lab.station_hours(day, 24, air + 2.0).await;
    }
}

async fn outlook(state: &AppState) -> Result<SoilTempForecast, TurfOpsError> {
    get_soil_temp_forecast(State(state.clone()))
        .await
        .map(|Json(f)| f)
}

// @spec WX-OUTLOOK-METHOD-001, WX-OUTLOOK-API-001, WX-OUTLOOK-MODEL-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn measured_soil_is_fitted_and_starts_from_its_station(pool: PgPool) {
    let lab = Lab::new(pool.clone()).await;
    measured_july(&lab).await;
    let f = outlook(&state(pool, ten_days()).await).await.unwrap();
    assert_eq!(f.method, OutlookMethod::Fitted);
    assert_eq!(f.basis.as_ref().map(|b| b.kind), Some(BasisKind::Station));
    let info = f.model_info.expect("a fit");
    assert!(info.r_squared >= 0.3, "{info:?}");
    assert!(f.estimate_info.is_none());
    assert!(
        f.predictions.iter().all(|p| p.date <= d(29)),
        "ten forecast days"
    );
}

// @spec WX-OUTLOOK-API-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn no_location_and_no_forecast_are_503s_with_their_own_sentences(pool: PgPool) {
    let mut profile =
        crate::logic::rules::test_fixtures::profile(crate::models::GrassType::TallFescue);
    profile.id = None;
    crate::db::queries::create_lawn_profile(&pool, &profile)
        .await
        .unwrap();
    let err = outlook(&state(pool, ten_days()).await).await.unwrap_err();
    assert_eq!(
        err.owner_message(),
        TurfOpsError::location_not_set().owner_message()
    );
    assert_eq!(err.into_response().status(), 503);
}

// @spec WX-OUTLOOK-API-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_no_full_forecast_day_the_outlook_names_the_forecast(pool: PgPool) {
    let lab = Lab::new(pool.clone()).await;
    measured_july(&lab).await;
    let err = outlook(&state(pool, ForecastSlot::new()).await)
        .await
        .unwrap_err();
    assert!(
        err.owner_message().contains("forecast"),
        "{}",
        err.owner_message()
    );
    assert_eq!(err.into_response().status(), 503);
}

// @spec WX-OUTLOOK-API-003, WX-OUTLOOK-METHOD-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_no_measured_soil_and_no_estimate_there_is_not_enough_data(pool: PgPool) {
    let lab = Lab::new(pool.clone()).await;
    for day in 1..=19 {
        lab.grid(PRISM, day, 80.0, 60.0).await;
    }
    let err = outlook(&state(pool, ten_days()).await).await.unwrap_err();
    assert!(
        err.owner_message()
            .starts_with("Not enough recent soil data"),
        "{}",
        err.owner_message()
    );
    assert_eq!(err.into_response().status(), 503);
}
