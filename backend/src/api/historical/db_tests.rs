//! `GET /api/v1/historical` from the weather record (`--features db-tests`).

use super::*;
use crate::logic::data_sync::DataSyncService;
use crate::logic::weather_record::lab::{d, Lab, PRISM};
use crate::models::weather_basis::BasisKind;
use axum::response::IntoResponse;
use chrono::TimeZone;
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

async fn historical(state: &AppState, range: &str) -> Result<HistoricalData, TurfOpsError> {
    get_historical(
        State(state.clone()),
        Query(HistoricalQuery {
            range: Some(range.into()),
        }),
    )
    .await
    .map(|Json(data)| data)
}

// @spec WX-VIEW-API-002, WX-VIEW-API-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_range_is_the_records_hours_with_each_series_basis(pool: PgPool) {
    let lab = Lab::new(pool.clone()).await;
    for day in 14..=19 {
        lab.station_hours(day, 24, 70.0).await;
        lab.grid(PRISM, day, 84.0, 64.0).await;
    }
    let data = historical(&state(pool).await, "7d").await.unwrap();

    assert!(!data.soil_temp_10_f.is_empty());
    assert!(data
        .soil_temp_10_f
        .windows(2)
        .all(|w| w[0].timestamp < w[1].timestamp));
    assert!(data.soil_temp_10_f.iter().all(|p| p.value == 70.0));
    let soil: Vec<&SeriesSpan> = data
        .basis
        .iter()
        .filter(|s| s.series == "soil_temp_10_f")
        .collect();
    assert_eq!(soil.len(), 1, "{:?}", data.basis);
    assert_eq!(soil[0].basis.kind, BasisKind::Station);
    assert_eq!((soil[0].from, soil[0].to), (d(14), d(19)));

    // (84 + 64) / 2 − 50 = 24 GDD a day, accumulated from Jan 1.
    let last = data.gdd_accumulation.last().unwrap();
    assert_eq!(last.value, 6.0 * 24.0);
    assert!(data
        .basis
        .iter()
        .any(|s| s.series == "gdd_accumulation" && s.basis.kind == BasisKind::Estimate));
}

// @spec WX-VIEW-API-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn another_range_is_a_400_and_no_location_a_503(pool: PgPool) {
    let mut profile =
        crate::logic::rules::test_fixtures::profile(crate::models::GrassType::TallFescue);
    profile.id = None;
    crate::db::queries::create_lawn_profile(&pool, &profile)
        .await
        .unwrap();
    let state = state(pool).await;
    let bad = historical(&state, "14d").await.unwrap_err();
    assert_eq!(bad.into_response().status(), 400);
    let unplaced = historical(&state, "7d").await.unwrap_err();
    assert_eq!(
        unplaced.owner_message(),
        TurfOpsError::location_not_set().owner_message()
    );
    assert_eq!(unplaced.into_response().status(), 503);
}
