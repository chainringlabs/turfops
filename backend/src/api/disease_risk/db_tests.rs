//! `GET /disease-risk` and the feed bridge against the weather record (`--features db-tests`).

use super::*;
use crate::clock::Clock;
use crate::logic::data_sync::DataSyncService;
use crate::logic::weather_record::lab::{d, Lab};
use crate::models::weather_basis::BasisKind;
use crate::models::RiskTier;
use axum::response::IntoResponse;
use chrono::{TimeZone, Utc};
use sqlx::PgPool;

/// Noon Eastern on July 20, 2026, the Lab's "now".
pub(super) fn clock() -> Clock {
    Clock::Frozen {
        at: Utc.with_ymd_and_hms(2026, 7, 20, 16, 0, 0).unwrap(),
        zone: chrono_tz::America::New_York,
    }
}

pub(super) async fn state_on(pool: &PgPool, clock: Clock) -> AppState {
    let mut config = crate::logic::data_sync::test_env::config(&[]);
    config.clock = clock;
    let sync = DataSyncService::initialize(&config, pool.clone()).await;
    AppState::new(pool.clone(), sync, None, false, clock)
}

pub(super) async fn profile(pool: &PgPool) -> LawnProfile {
    queries::get_default_lawn_profile(pool)
        .await
        .unwrap()
        .unwrap()
}

/// Warm, saturated hours from the hourly analysis: every hour of July 8–19 and the
/// first twelve hours of the 20th.
pub(super) async fn humid_analysis(lab: &Lab) {
    for day in 8..=19 {
        lab.analysis_hours(day, 24, 80.0, 95.0, 0.0).await;
    }
    lab.analysis_hours(20, 12, 80.0, 95.0, 0.0).await;
}

// @spec DIS-MODEL-API-001, DIS-MODEL-API-006, DIS-MGMT-REC-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_page_and_the_feed_read_the_same_approximate_assessment(pool: PgPool) {
    let lab = Lab::new(pool.clone()).await;
    humid_analysis(&lab).await;
    let state = state_on(&pool, clock()).await;
    let profile = profile(&pool).await;

    let Json(response) = get_disease_risk(State(state.clone())).await.unwrap();
    let recs = recommendations(&state, &profile).await;

    assert_eq!(response.today, d(20));
    assert_eq!(response.generated_at, clock().now());
    assert_eq!(response.diseases.len(), 5);
    // Today has no afternoon yet: the headline is the 19th, from the hourly analysis.
    assert!(response.approximate);
    let headline = response.basis.headline.as_ref().unwrap();
    assert_eq!(headline.kind, BasisKind::Estimate);
    assert!(headline.approximate);
    let spans: Vec<_> = response
        .basis
        .spans
        .iter()
        .map(|s| (s.from, s.to))
        .collect();
    assert_eq!(spans, vec![(d(14), d(20))]);
    for risk in &response.diseases {
        assert_eq!(risk.as_of, d(19), "{}", risk.name);
        assert!(risk.approximate, "{}", risk.name);
        assert!(risk.daily.iter().all(|day| day.approximate));
    }
    assert!(response.data_notes.iter().any(|n| n == APPROXIMATE_NOTE));
    assert!(response
        .data_notes
        .iter()
        .any(|n| n.contains("Forecast unavailable")));

    let mut from_page: Vec<String> = response
        .diseases
        .iter()
        .filter(|r| r.tier >= RiskTier::High)
        .map(|r| format!("disease_{}", r.slug.replace('-', "_")))
        .collect();
    let mut from_feed: Vec<String> = recs.iter().map(|r| r.id.clone()).collect();
    from_page.sort();
    from_feed.sort();
    assert!(!from_page.is_empty());
    assert_eq!(from_page, from_feed);
    for rec in &recs {
        let risk = response
            .diseases
            .iter()
            .find(|r| rec.id.ends_with(&r.slug.replace('-', "_")))
            .unwrap();
        assert_eq!(rec.description, risk.summary);
        assert!(
            rec.data_points.iter().any(|p| p.label == "Weather basis"),
            "{}",
            rec.id
        );
    }
}

// @spec DIS-MODEL-DAY-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn today_is_the_lawns_date_even_with_no_hours(pool: PgPool) {
    Lab::new(pool.clone()).await;
    // 10 p.m. Eastern on the 20th is already the 21st in UTC.
    let clock = Clock::Frozen {
        at: Utc.with_ymd_and_hms(2026, 7, 21, 2, 0, 0).unwrap(),
        zone: chrono_tz::America::New_York,
    };
    let state = state_on(&pool, clock).await;

    let Json(response) = get_disease_risk(State(state)).await.unwrap();

    assert_eq!(response.today, d(20));
    assert!(response.diseases.is_empty());
    assert!(!response.approximate);
    assert!(response.basis.headline.is_none());
    assert!(response
        .data_notes
        .contains(&"No recent weather observations are available.".to_string()));
}

// @spec DIS-MODEL-API-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn no_profile_is_404_and_no_location_is_the_records_503(pool: PgPool) {
    let state = state_on(&pool, clock()).await;
    let status = |r: Result<Json<DiseaseRiskResponse>, TurfOpsError>| {
        r.unwrap_err().into_response().status().as_u16()
    };
    assert_eq!(status(get_disease_risk(State(state.clone())).await), 404);

    let mut unplaced =
        crate::logic::rules::test_fixtures::profile(crate::models::GrassType::TallFescue);
    unplaced.id = None;
    queries::create_lawn_profile(&pool, &unplaced)
        .await
        .unwrap();
    let err = get_disease_risk(State(state)).await.unwrap_err();
    assert_eq!(
        err.owner_message(),
        TurfOpsError::location_not_set().owner_message()
    );
    assert_eq!(err.into_response().status(), 503);
}

// @spec DIS-MGMT-REC-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_failed_assessment_contributes_no_recommendations(pool: PgPool) {
    let mut unplaced =
        crate::logic::rules::test_fixtures::profile(crate::models::GrassType::TallFescue);
    unplaced.id = None;
    queries::create_lawn_profile(&pool, &unplaced)
        .await
        .unwrap();
    let state = state_on(&pool, clock()).await;
    let profile = profile(&pool).await;
    assert!(recommendations(&state, &profile).await.is_empty());
}
