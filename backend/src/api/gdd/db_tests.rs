//! `GET /api/v1/gdd` from the weather record (`--features db-tests`).

use super::*;
use crate::clock::Clock;
use crate::logic::data_sync::DataSyncService;
use crate::logic::weather_record::lab::{d, Lab, PRISM};
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

async fn state(pool: PgPool) -> AppState {
    let mut config = crate::logic::data_sync::test_env::config(&[]);
    config.clock = clock();
    let sync = DataSyncService::initialize(&config, pool.clone()).await;
    AppState::new(pool, sync, None, false, clock())
}

async fn gdd(state: &AppState, year: Option<i32>) -> Result<GddSummary, TurfOpsError> {
    get_gdd(State(state.clone()), Query(GddQuery { year }))
        .await
        .map(|Json(summary)| summary)
}

// @spec RULE-GDD-001, RULE-GDD-006, WX-VIEW-BASIS-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn gdd_sums_the_records_daily_gdd50_through_today(pool: PgPool) {
    let lab = Lab::new(pool.clone()).await;
    // (80 + 60) / 2 − 50 = 20 GDD on each of July 17 and 18; a cold day adds nothing.
    lab.grid(PRISM, 17, 80.0, 60.0).await;
    lab.grid(PRISM, 18, 80.0, 60.0).await;
    lab.grid(PRISM, 19, 50.0, 30.0).await;
    let state = state(pool).await;

    let summary = gdd(&state, None).await.unwrap();
    assert_eq!(summary.year, 2026);
    assert_eq!(summary.current_gdd_total, 40.0);
    assert_eq!(summary.last_computed_date, Some(d(19)));
    let running: Vec<f64> = summary
        .daily_history
        .iter()
        .map(|day| day.cumulative_gdd_base50)
        .collect();
    assert_eq!(running, vec![20.0, 40.0, 40.0]);
    // @spec WX-VIEW-BASIS-001: the total names the daily air it was summed from.
    let basis = summary.basis.expect("days, so a basis");
    assert_eq!(basis.basis.source, lab.source(PRISM).await.name);
    assert!(!basis.mixed);

    let err = gdd(&state, Some(300_000)).await.unwrap_err();
    assert_eq!(err.into_response().status(), 400);
}

// @spec RULE-GDD-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_no_location_gdd_is_unavailable_not_zero(pool: PgPool) {
    let mut profile =
        crate::logic::rules::test_fixtures::profile(crate::models::GrassType::TallFescue);
    profile.id = None;
    crate::db::queries::create_lawn_profile(&pool, &profile)
        .await
        .unwrap();
    let err = gdd(&state(pool).await, None).await.unwrap_err();
    assert!(matches!(err, TurfOpsError::Unavailable { .. }), "{err:?}");
    assert_eq!(err.into_response().status(), 503);
}
