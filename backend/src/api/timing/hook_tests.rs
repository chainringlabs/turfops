//! The timing windows' hooks into the recommendation feed and the seasonal plan.

use super::db_harness::{app_state, climate, forecast_body, lawn, ZONE};
use super::*;
use crate::models::seasonal_plan::ActivityStatus;
use crate::models::timing::{WindowId, WindowState};
use crate::models::{RecommendationCategory, Severity};
use crate::test_support::stub_http;
use sqlx::PgPool;

fn fall_herbicide() -> Recommendation {
    Recommendation::new(
        "broadleaf_fall",
        RecommendationCategory::Herbicide,
        Severity::Warning,
        "Fall Broadleaf Herbicide",
        "Best window of the year.",
    )
}

// @spec TIME-FEED-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn an_unanswering_record_leaves_the_feed_as_the_rules_made_it(pool: PgPool) {
    // A lawn with no location: the weather record cannot answer.
    let unplaced = crate::models::LawnProfile::new(
        "Test".into(),
        crate::models::GrassType::TallFescue,
        "7a".into(),
    );
    queries::create_lawn_profile(&pool, &unplaced)
        .await
        .unwrap();
    let profile = lawn(&pool).await;
    let state = app_state(&pool, None).await;
    let mut recs = vec![fall_herbicide()];
    apply_to_feed(&state, &profile, &[], &mut recs).await;
    assert_eq!(recs.len(), 1, "no timing or aeration recommendation added");
    assert_eq!(recs[0].severity, Severity::Warning, "not reconciled");
    assert_eq!(recs[0].description, "Best window of the year.");
}

// @spec TIME-PLAN-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn without_an_answer_or_a_history_the_plan_keeps_its_own_activities(pool: PgPool) {
    let year = ZONE.today().year();
    let yesterday = ZONE.today() - Duration::days(1);

    // Only this year on record: no typical dates, so no activities.
    climate(&pool, 0, yesterday).await;
    let mut profile = lawn(&pool).await;
    let state = app_state(&pool, None).await;
    assert!(plan_activities(&state, &profile, year).await.is_none());

    // The record cannot answer once the lawn has no location.
    profile.location = None;
    queries::update_lawn_profile(&pool, &profile).await.unwrap();
    let state = app_state(&pool, None).await;
    assert!(plan_activities(&state, &profile, year).await.is_none());
}

// @spec TIME-PLAN-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_a_history_the_plan_gets_timing_activities(pool: PgPool) {
    climate(&pool, 6, ZONE.today() - Duration::days(1)).await;
    let profile = lawn(&pool).await;
    let state = app_state(&pool, None).await;
    let year = ZONE.today().year();
    assert!(plan_activities(&state, &profile, year).await.is_some());
}

fn activity_id(id: WindowId) -> Option<&'static str> {
    match id {
        WindowId::SpringPreEmergent => Some("pre_emergent"),
        WindowId::FallPreEmergent => Some("fall_pre_emergent"),
        WindowId::FallSeeding => Some("fall_overseeding"),
        _ => None,
    }
}

fn status_for(state: WindowState) -> Option<ActivityStatus> {
    match state {
        WindowState::NotYet | WindowState::OpeningSoon => Some(ActivityStatus::Upcoming),
        WindowState::Open | WindowState::Ideal | WindowState::Closing => {
            Some(ActivityStatus::Active)
        }
        WindowState::Closed => Some(ActivityStatus::Missed),
        WindowState::Done => Some(ActivityStatus::Completed),
        WindowState::Blocked => None,
    }
}

// @spec TIME-PLAN-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_plan_reads_the_windows_as_the_timing_page_does(pool: PgPool) {
    climate(&pool, 6, ZONE.today() - Duration::days(1)).await;
    let profile = lawn(&pool).await;
    // A full week of forecast, so the soil outlook runs.
    let week: Vec<(i64, Vec<f64>)> = (0..7)
        .map(|d| (d, vec![50.0, 48.0, 52.0, 60.0, 66.0, 64.0, 58.0, 54.0]))
        .collect();
    let base = stub_http(200, forecast_body(&week)).await;
    let state = app_state(&pool, Some(&base)).await;

    let page = compute(&state, &profile).await.unwrap();
    assert!(
        page.series.iter().any(|d| d.forecast.is_some()),
        "the page has a soil outlook"
    );
    let year = page.today.year();
    let plan = plan_activities(&state, &profile, year).await.unwrap();

    let mut compared = 0;
    for window in page.windows.iter().filter(|w| w.season_year == year) {
        let Some(id) = activity_id(window.id) else {
            continue;
        };
        let status = plan.iter().find(|a| a.id == id).map(|a| a.status);
        assert_eq!(status, status_for(window.state), "{id}: {:?}", window.state);
        compared += 1;
    }
    assert!(compared >= 2, "the fall windows are always this year's");
}
