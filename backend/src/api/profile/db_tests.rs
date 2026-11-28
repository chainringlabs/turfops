//! `/api/v1/profile` against PostgreSQL.

use super::{get_profile, update_profile, UpdateProfileRequest};
use crate::api::applications::db_support::{lawn, reply, state};
use crate::models::{GrassType, IrrigationType, LawnProfile, SoilType};
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::json;
use sqlx::PgPool;

async fn put(
    state: &AppState,
    body: serde_json::Value,
) -> Result<LawnProfile, (StatusCode, String)> {
    let req: UpdateProfileRequest = serde_json::from_value(body).unwrap();
    match update_profile(State(state.clone()), Json(req)).await {
        Ok(Json(profile)) => Ok(profile),
        Err(err) => Err(reply(err).await),
    }
}

async fn profiles(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM lawn_profiles")
        .fetch_one(pool)
        .await
        .unwrap()
}

// @spec APP-PROF-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_profile_or_a_404_that_says_none_exists(pool: PgPool) {
    let state = state(&pool).await;
    let err = get_profile(State(state.clone())).await.unwrap_err();
    assert_eq!(
        reply(err).await,
        (StatusCode::NOT_FOUND, "No lawn profile found".to_string())
    );
    let id = lawn(&pool).await;
    let Json(profile) = get_profile(State(state)).await.unwrap();
    assert_eq!(profile.id, Some(id));
}

// @spec APP-PROF-002, APP-PROF-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn every_profile_field_is_stored_by_its_variant_name(pool: PgPool) {
    lawn(&pool).await;
    let state = state(&pool).await;
    let saved = put(
        &state,
        json!({
            "name": "Back Forty", "grass_type": "kbg", "usda_zone": "zone six-ish",
            "soil_type": "silt loam", "lawn_size_sqft": 4200.0, "irrigation_type": "in ground"
        }),
    )
    .await
    .unwrap();
    assert_eq!(saved.name, "Back Forty");
    assert_eq!(saved.grass_type, GrassType::KentuckyBluegrass);
    assert_eq!(saved.usda_zone, "zone six-ish", "the zone is free text");
    assert_eq!(saved.soil_type, Some(SoilType::SiltLoam));
    assert_eq!(saved.lawn_size_sqft, Some(4200.0));
    assert_eq!(saved.irrigation_type, Some(IrrigationType::InGround));

    let stored: (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT grass_type, soil_type, irrigation_type FROM lawn_profiles")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored,
        (
            "KentuckyBluegrass".into(),
            Some("SiltLoam".into()),
            Some("InGround".into())
        )
    );
    let body = serde_json::to_value(&saved).unwrap();
    assert_eq!(body["grass_type"], "KentuckyBluegrass");
    assert_eq!(body["soil_type"], "SiltLoam");
    assert_eq!(body["irrigation_type"], "InGround");
}

// @spec APP-PROF-009
#[sqlx::test(migrations = "src/db/migrations")]
async fn an_update_touches_only_the_fields_sent(pool: PgPool) {
    lawn(&pool).await;
    let state = state(&pool).await;
    put(
        &state,
        json!({ "soil_type": "Clay", "lawn_size_sqft": 5000.0, "irrigation_type": "Hose" }),
    )
    .await
    .unwrap();

    let renamed = put(&state, json!({ "name": "Side Yard" })).await.unwrap();
    assert_eq!(renamed.name, "Side Yard");
    assert_eq!(renamed.usda_zone, "7a", "absent = left alone");
    assert_eq!(renamed.soil_type, Some(SoilType::Clay));
    assert_eq!(renamed.lawn_size_sqft, Some(5000.0));

    let cleared = put(
        &state,
        json!({ "soil_type": "", "irrigation_type": "", "lawn_size_sqft": 0.0 }),
    )
    .await
    .unwrap();
    assert_eq!(cleared.soil_type, None);
    assert_eq!(cleared.irrigation_type, None);
    assert_eq!(cleared.lawn_size_sqft, None);
    assert_eq!(cleared.name, "Side Yard");
    let negative = put(&state, json!({ "lawn_size_sqft": 4000.0 }))
        .await
        .unwrap();
    assert_eq!(negative.lawn_size_sqft, Some(4000.0));
    assert_eq!(
        put(&state, json!({ "lawn_size_sqft": -1.0 }))
            .await
            .unwrap()
            .lawn_size_sqft,
        None
    );

    let stored_at: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT updated_at FROM lawn_profiles")
            .fetch_one(&pool)
            .await
            .unwrap();
    let last = put(&state, json!({})).await.unwrap();
    let reread: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT updated_at FROM lawn_profiles")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(reread >= stored_at);
    assert_eq!(last.updated_at, reread, "the response is the stored row");
}

// @spec APP-PROF-010
#[sqlx::test(migrations = "src/db/migrations")]
async fn an_unknown_type_is_a_400_naming_it_and_changes_nothing(pool: PgPool) {
    lawn(&pool).await;
    let state = state(&pool).await;
    for (field, value) in [
        ("grass_type", "Bluegras"),
        ("soil_type", "Mud"),
        ("irrigation_type", "drip"),
    ] {
        let (status, msg) = put(&state, json!({ "name": "Renamed", field: value }))
            .await
            .unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST, "{field}");
        assert!(msg.contains(value), "{msg}");
    }
    let Json(profile) = get_profile(State(state)).await.unwrap();
    assert_eq!(profile.name, "Front", "nothing changed");
}

// @spec APP-PROF-011, APP-PROF-012
#[sqlx::test(migrations = "src/db/migrations")]
async fn saving_with_no_profile_creates_it_with_the_seeding_defaults(pool: PgPool) {
    let state = state(&pool).await;
    let (status, _) = put(&state, json!({ "grass_type": "Bluegras" }))
        .await
        .unwrap_err();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(profiles(&pool).await, 0, "a 400 creates nothing");

    let created = put(&state, json!({ "usda_zone": "6b", "soil_type": "" }))
        .await
        .unwrap();
    assert!(created.id.is_some(), "the stored row");
    assert_eq!(created.name, "Main Lawn");
    assert_eq!(created.grass_type, GrassType::TallFescue);
    assert_eq!(created.usda_zone, "6b");
    assert_eq!(
        (
            created.soil_type,
            created.lawn_size_sqft,
            created.irrigation_type
        ),
        (None, None, None)
    );

    put(&state, json!({ "name": "Front" })).await.unwrap();
    assert_eq!(profiles(&pool).await, 1, "the next save updates it");
}

// @spec APP-PROF-013, APP-PROF-014
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_location_is_stored_rounded_with_the_timezone_that_follows_from_it(pool: PgPool) {
    lawn(&pool).await;
    let state = state(&pool).await;
    let before = put(&state, json!({})).await.unwrap();
    assert_eq!(before.location, None, "a profile starts without a location");

    let saved = put(
        &state,
        json!({"location": {"latitude": 39.861_234_9, "longitude": -75.786_551,
                             "place_name": " Avondale, PA "}}),
    )
    .await
    .unwrap();
    let location = saved.location.expect("the location");
    assert_eq!((location.latitude, location.longitude), (39.8612, -75.7866));
    assert_eq!(location.place_name.as_deref(), Some("Avondale, PA"));
    assert_eq!(location.timezone, "America/New_York");

    // A later save that says nothing about the location leaves it alone.
    let renamed = put(&state, json!({"name": "Back Forty"})).await.unwrap();
    assert_eq!(renamed.location.unwrap().timezone, "America/New_York");
}

// @spec APP-PROF-014
#[sqlx::test(migrations = "src/db/migrations")]
async fn moving_the_lawn_moves_its_timezone_and_a_request_cannot_set_one(pool: PgPool) {
    lawn(&pool).await;
    let state = state(&pool).await;
    let moved = put(
        &state,
        json!({"location": {"latitude": 33.45, "longitude": -112.07,
                             "timezone": "America/New_York"}}),
    )
    .await
    .unwrap();
    assert_eq!(moved.location.unwrap().timezone, "America/Phoenix");
}

// @spec APP-PROF-017
#[sqlx::test(migrations = "src/db/migrations")]
async fn half_a_location_or_one_outside_the_contiguous_us_is_a_400_that_changes_nothing(
    pool: PgPool,
) {
    lawn(&pool).await;
    let state = state(&pool).await;
    put(
        &state,
        json!({"location": {"latitude": 39.86, "longitude": -75.79}}),
    )
    .await
    .unwrap();
    for (body, says) in [
        (
            json!({"name": "Changed", "location": {"latitude": 40.0}}),
            "longitude",
        ),
        (
            json!({"name": "Changed", "location": {"longitude": -80.0}}),
            "latitude",
        ),
        (
            json!({"name": "Changed", "location": {"latitude": 61.2, "longitude": -149.9}}),
            "contiguous United States",
        ),
        (
            json!({"name": "Changed", "location": {"latitude": 0.0, "longitude": 0.0}}),
            "contiguous United States",
        ),
    ] {
        let (status, message) = put(&state, body).await.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(message.contains(says), "{message}");
    }
    let Json(after) = get_profile(State(state)).await.unwrap();
    assert_ne!(after.name, "Changed");
    assert_eq!(after.location.unwrap().latitude, 39.86);
}

// @spec APP-PROF-013
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_place_name_can_change_without_moving_the_lawn(pool: PgPool) {
    lawn(&pool).await;
    let state = state(&pool).await;
    put(
        &state,
        json!({"location": {"latitude": 39.86, "longitude": -75.79, "place_name": "Home"}}),
    )
    .await
    .unwrap();
    let renamed = put(&state, json!({"location": {"place_name": "The farm"}}))
        .await
        .unwrap()
        .location
        .unwrap();
    assert_eq!(
        (renamed.latitude, renamed.place_name.as_deref()),
        (39.86, Some("The farm"))
    );

    // A place name alone, on a lawn with no location, has nowhere to go.
    sqlx::query("UPDATE lawn_profiles SET latitude = NULL, longitude = NULL, timezone = NULL")
        .execute(&pool)
        .await
        .unwrap();
    let (status, message) = put(&state, json!({"location": {"place_name": "Nowhere"}}))
        .await
        .unwrap_err();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        message.contains("latitude") && message.contains("longitude"),
        "{message}"
    );
}

// @spec APP-PROF-012, APP-PROF-013
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_profile_created_by_a_save_can_carry_its_location(pool: PgPool) {
    let state = state(&pool).await;
    let created = put(
        &state,
        json!({"location": {"latitude": 41.88, "longitude": -87.63}}),
    )
    .await
    .unwrap();
    assert_eq!(created.location.unwrap().timezone, "America/Chicago");
    assert_eq!(profiles(&pool).await, 1);
}

// @spec DEPLOY-TZ-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_new_location_puts_its_zone_on_the_apps_clock(pool: PgPool) {
    lawn(&pool).await;
    let state = state(&pool).await;
    state.clock.replace(crate::clock::Clock::System);
    // Denver, CO.
    put(
        &state,
        json!({"location": {"latitude": 39.74, "longitude": -104.99}}),
    )
    .await
    .unwrap();
    assert_eq!(
        state.clock.get(),
        crate::clock::Clock::Zone(chrono_tz::America::Denver)
    );
}

/// The sources the stored plan holds, by key.
async fn planned(pool: &PgPool) -> Vec<String> {
    let mut keys: Vec<String> = crate::db::weather_queries::plan::read(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.source.key)
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

// @spec SYNC-SCHED-008, APP-PROF-018
#[sqlx::test(migrations = "src/db/migrations")]
async fn moving_the_lawn_rebuilds_its_plan_and_wakes_the_schedule(pool: PgPool) {
    lawn(&pool).await;
    let (_schedule, waker, _stopper) = crate::logic::weather_sync::schedule::Schedule::new(
        pool.clone(),
        crate::logic::weather_sync::fetcher::Clients::new(),
    );
    let state = state(&pool).await.waking(waker.clone());
    assert!(planned(&pool).await.is_empty(), "no location, no plan");

    put(
        &state,
        json!({"location": {"latitude": 39.86, "longitude": -75.79}}),
    )
    .await
    .unwrap();
    let here = planned(&pool).await;
    assert!(!here.is_empty(), "a plan for the new location");
    assert!(waker.take_wake().await, "the schedule is woken");

    // Saves that leave the point where it is wake nothing.
    put(&state, json!({"name": "Back Forty"})).await.unwrap();
    put(
        &state,
        json!({"location": {"latitude": 39.86, "longitude": -75.79, "place_name": "Home"}}),
    )
    .await
    .unwrap();
    assert!(!waker.take_wake().await, "the lawn did not move");

    put(
        &state,
        json!({"location": {"latitude": 33.45, "longitude": -112.07}}),
    )
    .await
    .unwrap();
    assert!(waker.take_wake().await, "moved again");
    assert_ne!(planned(&pool).await, Vec::<String>::new());
}
