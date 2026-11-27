//! Fixtures for the PostgreSQL-backed seasonal plan and calendar tests: an `AppState` with
//! or without a current reading, and crossings held as if found in the weather record.

use crate::clock::Clock;
use crate::db::queries;
use crate::error::TurfOpsError;
pub(crate) use crate::logic::data_sync::test_support::Fixed;
use crate::logic::data_sync::DataSyncService;
use crate::logic::seasonal_plan::RecordCrossings;
use crate::models::seasonal_plan::{SeasonalPlan, ThresholdCrossing};
use crate::models::{Application, ApplicationType, GrassType, LawnProfile};
use crate::state::AppState;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::{Datelike, NaiveDate};
use sqlx::PgPool;
use std::sync::Arc;

pub(crate) fn d(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

pub(crate) fn clock() -> Clock {
    Clock::Zone(chrono_tz::UTC)
}

pub(crate) fn this_year() -> i32 {
    clock().today().year()
}

/// App state on `pool` whose data sync answers with `fixed`.
pub(crate) fn state(pool: &PgPool, fixed: Fixed) -> AppState {
    let sync = DataSyncService::for_tests(pool.clone(), clock(), fixed);
    AppState::new(pool.clone(), sync, None, false, clock())
}

/// Three settled years (`plan_year` − 3 … − 1) of every threshold the 10 cm activities use,
/// each on the same month and day every year, held as the record's crossings for `state`.
pub(crate) async fn settled_history(state: &AppState, plan_year: i32) {
    let thresholds = [
        ("soil_50f_rising", 4, 6),
        ("soil_55f_rising", 4, 23),
        ("soil_60f_rising", 5, 12),
        ("soil_65f_rising", 5, 27),
        ("soil_75f_rising", 6, 27),
        ("soil_65f_falling", 9, 25),
        ("soil_55f_falling", 10, 27),
        ("soil_50f_falling", 11, 7),
        ("soil_45f_falling", 11, 22),
    ];
    let mut found = RecordCrossings::default();
    for year in plan_year - 3..plan_year {
        for (name, month, day) in thresholds {
            found.crossings.push(ThresholdCrossing {
                year,
                threshold_name: name.into(),
                crossing_date: d(year, month, day),
                avg_soil_temp_f: 50.0,
                estimated: false,
            });
        }
        found.settled_years.push(year);
    }
    *state.crossing_memo.write().await = Some((None, chrono::Utc::now(), Arc::new(found)));
}

pub(crate) async fn create_profile(pool: &PgPool, name: &str) -> i64 {
    let profile = LawnProfile::new(name.to_string(), GrassType::TallFescue, "7a".to_string());
    queries::create_lawn_profile(pool, &profile).await.unwrap()
}

pub(crate) fn application(profile: i64, kind: ApplicationType, on: NaiveDate) -> Application {
    Application {
        id: None,
        lawn_profile_id: profile,
        application_type: kind,
        product_name: None,
        application_date: on,
        rate_per_1000sqft: None,
        coverage_sqft: None,
        notes: None,
        plant_name: None,
        nitrogen_pct: None,
        phosphorus_pct: None,
        potassium_pct: None,
        plant_id: None,
        follow_up_date: None,
        frac_classes: None,
        product_id: None,
        created_at: chrono::Utc::now(),
    }
}

pub(crate) async fn log(pool: &PgPool, app: Application) -> i64 {
    queries::create_application(pool, &app).await.unwrap()
}

/// The status code a handler error becomes.
pub(crate) fn status_of(error: TurfOpsError) -> StatusCode {
    error.into_response().status()
}

/// `GET /api/v1/seasonal-plan`, optionally with `?year=`.
pub(crate) async fn get_plan(
    state: &AppState,
    year: Option<i32>,
) -> Result<SeasonalPlan, StatusCode> {
    use super::{get_seasonal_plan, SeasonalPlanQuery};
    use axum::extract::{Query, State};
    get_seasonal_plan(State(state.clone()), Query(SeasonalPlanQuery { year }))
        .await
        .map(|json| json.0)
        .map_err(status_of)
}
