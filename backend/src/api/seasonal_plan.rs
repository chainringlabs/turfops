use crate::db::soil_estimate_queries;

use crate::db::{plant_queries, queries};
use crate::error::TurfOpsError;
use crate::logic::data_sync::{plan_key, record_asked};
use crate::logic::plant_maintenance::{build_plant_activities, plant_log_range};
use crate::logic::rules::plan_state::{season_states, SeasonInputs, SeasonState};
use crate::logic::seasonal_plan::{
    apply_rule_states, build_seasonal_plan, crossings_from_record, RecordCrossings,
};
use crate::logic::soil_estimate::scores::{self, Direction};
use crate::logic::weather_record::record::WeatherRecord;
use crate::models::seasonal_plan::SeasonalPlan;
use crate::models::LawnProfile;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::Json;
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use std::sync::Arc;

#[cfg(all(test, feature = "db-tests"))]
pub(crate) mod db_support;
#[cfg(all(test, feature = "db-tests"))]
mod merge_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod plant_overlay_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod record_db_tests;

#[derive(Debug, Deserialize)]
pub struct SeasonalPlanQuery {
    pub year: Option<i32>,
}

/// GET /api/v1/seasonal-plan?year=2026
///
/// Returns a seasonal plan for the requested year from the lawn's 10 cm threshold
/// crossings in the weather record.
// @spec PLAN-MERGE-001, PLAN-MERGE-002, PLAN-MERGE-003, PLAN-MERGE-004, PLAN-MERGE-005, PLAN-MERGE-007, PLAN-MERGE-008
pub async fn get_seasonal_plan(
    State(state): State<AppState>,
    Query(params): Query<SeasonalPlanQuery>,
) -> Result<Json<SeasonalPlan>, TurfOpsError> {
    let today = state.clock.today();
    let year = params.year.unwrap_or_else(|| today.year());
    let found = record_crossings(&state).await;

    // Get application history for the requested year
    let profile = queries::get_default_lawn_profile(&state.pool)
        .await?
        .ok_or_else(|| TurfOpsError::NotFound("No lawn profile found".into()))?;

    let profile_id = profile
        .id
        .ok_or_else(|| TurfOpsError::InvalidData("Profile missing ID".into()))?;

    let start_date = NaiveDate::from_ymd_opt(year, 1, 1)
        .ok_or_else(|| TurfOpsError::InvalidData(format!("Invalid year: {}", year)))?;
    let end_date = NaiveDate::from_ymd_opt(year + 1, 1, 1)
        .ok_or_else(|| TurfOpsError::InvalidData(format!("Invalid year: {}", year)))?;

    let applications = queries::get_applications_for_profile_in_range(
        &state.pool,
        profile_id,
        start_date,
        end_date,
    )
    .await?;

    let mut plan = build_seasonal_plan(
        year,
        &found.crossings,
        &found.settled_years,
        &applications,
        profile.grass_type,
        today,
    );
    if year == today.year() {
        let states = rule_states(&state, &profile, today).await;
        apply_rule_states(&mut plan.activities, states.as_deref());
    }

    // Pre-emergent, seeding and aeration come from the timing windows (5 cm soil +
    // freeze dates) so the plan agrees with the Timing page. The plan's own 10 cm version
    // of every activity the windows answer for is dropped — also one the log has ruled out
    // (seed vs. pre-emergent), which must not come back. When the weather record cannot
    // answer the originals stay.
    if let Some(timing) = super::timing::plan_activities(&state, &profile, year).await {
        let owned = crate::logic::timing::plan::owned_ids(profile.grass_type);
        plan.activities.retain(|a| !owned.contains(&a.id.as_str()));
        plan.activities.extend(timing);
    }

    // Overlay plant-maintenance activities from the landscape feature.
    // @spec PLAN-MERGE-006
    let plants = plant_queries::list_plants_for_profile(&state.pool, profile_id).await?;
    // A plant window can open late in the previous year (dormant pruning, Dec → Feb) or run
    // into the next; read the log from Oct 1 before through the end of the latest window.
    // @spec PLANT-PLAN-005
    let (plant_log_start, plant_log_end) = plant_log_range(&plants, year)
        .ok_or_else(|| TurfOpsError::InvalidData(format!("Invalid year: {}", year)))?;
    let plant_applications = queries::get_applications_for_profile_in_range(
        &state.pool,
        profile_id,
        plant_log_start,
        plant_log_end,
    )
    .await?;
    plan.activities.extend(build_plant_activities(
        &plants,
        &plant_applications,
        year,
        today,
    ));
    plan.activities
        .sort_by_key(|a| a.date_window.predicted_start);

    Ok(Json(plan))
}

/// How long the crossings found in the record are reused.
const CROSSINGS_FOR: chrono::Duration = chrono::Duration::hours(1);

/// The 10 cm crossings of the ten years before the lawn's current year and of the current
/// year, from one read of the weather record, reused for an hour while the basis plan and
/// the lawn's date stay the same. Nothing is stored. A record that cannot answer gives a
/// plan with no 10 cm crossings.
// @spec PLAN-CROSS-014, PLAN-CROSS-015
async fn record_crossings(state: &AppState) -> Arc<RecordCrossings> {
    match find_crossings(state).await {
        Ok(found) => found,
        Err(error) => {
            tracing::warn!(
                ?error,
                "Seasonal plan: no 10 cm crossings from the weather record"
            );
            Arc::new(RecordCrossings::default())
        }
    }
}

async fn find_crossings(state: &AppState) -> Result<Arc<RecordCrossings>, TurfOpsError> {
    let asked = record_asked(&state.pool, &state.clock.get()).await?;
    let today = asked.at.today;
    let key = format!("{today}|{}", plan_key(&state.pool).await?);
    let now = asked.at.now;
    if let Some((held, at, found)) = state.crossing_memo.read().await.as_ref() {
        let same = held.as_ref().is_none_or(|held| *held == key);
        if same && now - *at < CROSSINGS_FOR {
            return Ok(found.clone());
        }
    }
    let first = NaiveDate::from_ymd_opt(today.year() - 10, 1, 1)
        .ok_or_else(|| TurfOpsError::InvalidData(format!("Invalid year: {}", today.year())))?;
    let series = WeatherRecord::new(state.pool.clone())
        .fetch_daily_soil_temp_averages(first, today, asked)
        .await?;
    let calibration = soil_estimate_queries::all(&state.pool)
        .await?
        .into_iter()
        .find(|fit| fit.depth_cm == 10)
        .map(|fit| fit.calibration);
    let datable = |value: f64, falling: bool| {
        let direction = if falling {
            Direction::Falling
        } else {
            Direction::Rising
        };
        calibration.is_some_and(|calibration| {
            scores::embedded()
                .datable(10, value, direction, calibration)
                .is_some()
        })
    };
    let found = Arc::new(crossings_from_record(&series, today, &datable));
    *state.crossing_memo.write().await = Some((Some(key), now, found.clone()));
    Ok(found)
}

/// The current season's rule states, judged on the same feed, conditions and log as the
/// recommendations. `None` when the feed cannot be assembled (the plan then marks the
/// covered activities as estimates).
// @spec PLAN-ACT-021, PLAN-ACT-022
async fn rule_states(
    state: &AppState,
    profile: &LawnProfile,
    today: NaiveDate,
) -> Option<Vec<(&'static str, SeasonState)>> {
    let summary = {
        let mut service = state.sync_service.write().await;
        service.get_or_refresh().await
    };
    let summary = summary
        .inspect_err(|e| tracing::warn!("Seasonal plan: no conditions for rule states: {e}"))
        .ok()?;
    let (feed, history) = super::recommendation_feed::all_with_history(state, profile, &summary)
        .await
        .inspect_err(|e| tracing::warn!("Seasonal plan: no feed for rule states: {e}"))
        .ok()?;
    Some(season_states(&SeasonInputs {
        today,
        env: &summary,
        profile,
        history: &history,
        feed: &feed,
    }))
}
