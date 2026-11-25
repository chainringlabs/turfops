//! The seasonal plan's 10 cm crossings from the weather record (`--features db-tests`).

use super::*;
use crate::clock::Clock;
use crate::db::weather_queries::days;
use crate::logic::data_sync::DataSyncService;
use crate::logic::weather_record::lab::{Lab, STATION};
use crate::models::weather_record::HourlyRow;
use chrono::{Duration, TimeZone, Utc};
use chrono_tz::America::New_York;
use sqlx::PgPool;

/// Noon Eastern on July 20, 2026.
fn clock() -> Clock {
    Clock::Frozen {
        at: Utc.with_ymd_and_hms(2026, 7, 20, 16, 0, 0).unwrap(),
        zone: New_York,
    }
}

async fn state(pool: PgPool) -> AppState {
    let mut config = crate::logic::data_sync::test_env::config(&[]);
    config.clock = clock();
    let sync = DataSyncService::initialize(&config, pool.clone()).await;
    AppState::new(pool, sync, None, false, clock())
}

async fn plan(state: &AppState) -> SeasonalPlan {
    let Json(plan) = get_seasonal_plan(
        State(state.clone()),
        Query(SeasonalPlanQuery { year: None }),
    )
    .await
    .unwrap();
    plan
}

/// Every hour of 2025 from the station: 10 cm soil at 40°F through March, then 60°F, so
/// 50°F rising is crossed in April.
async fn measured_2025(lab: &Lab) {
    let start = days::start_of_day(New_York, NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
    let end = days::start_of_day(New_York, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    let mut rows = Vec::new();
    let mut at = start;
    while at < end {
        let local = at.with_timezone(&New_York).date_naive();
        let soil = if local.month() <= 3 { 40.0 } else { 60.0 };
        let mut row = HourlyRow {
            observed_at: at,
            ..Default::default()
        };
        row.soil_temp_f = [Some(soil + 2.0), Some(soil), None, None, None];
        rows.push(row);
        at += Duration::hours(1);
    }
    lab.write_hours(STATION, &rows).await;
}

// @spec PLAN-CROSS-014, PLAN-CROSS-010, PLAN-CROSS-012
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_plan_votes_with_the_records_settled_years_and_holds_them_an_hour(pool: PgPool) {
    let lab = Lab::new(pool.clone()).await;
    measured_2025(&lab).await;
    let state = state(pool).await;

    let first = plan(&state).await;
    assert_eq!(
        first.data_years_used, 1,
        "2025 is over and held through Dec 31"
    );
    assert_eq!(first.estimated_years, 0, "measured at the station");
    let memo = state.crossing_memo.read().await.clone().expect("held");
    assert!(memo
        .0
        .as_deref()
        .is_some_and(|key| key.starts_with("2026-07-20|")));
    assert!(memo
        .2
        .crossings
        .iter()
        .any(|c| c.year == 2025 && c.threshold_name == "soil_50f_rising"));

    // Within the hour the held crossings are reused, not read again.
    sqlx::query("DELETE FROM weather_hourly")
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(plan(&state).await.data_years_used, 1);
}

// @spec PLAN-CROSS-015
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_no_location_the_plan_has_no_10_cm_crossings_and_still_answers(pool: PgPool) {
    let mut profile =
        crate::logic::rules::test_fixtures::profile(crate::models::GrassType::TallFescue);
    profile.id = None;
    queries::create_lawn_profile(&pool, &profile).await.unwrap();
    let plan = plan(&state(pool).await).await;
    assert_eq!(plan.data_years_used, 0);
    assert_eq!(plan.estimated_years, 0);
}

// @spec PLAN-CROSS-016
#[sqlx::test(migrations = "src/db/migrations")]
async fn no_crossing_is_stored_in_the_database(pool: PgPool) {
    for table in ["seasonal_threshold_crossings", "seasonal_crossing_years"] {
        let exists: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
            .bind(table)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(exists, None, "{table} is dropped");
    }
}
