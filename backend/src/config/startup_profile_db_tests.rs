//! Seeding the lawn profile from `LAWN_*` at startup, against PostgreSQL.

use super::load::from_vars;
use super::Config;
use crate::db::queries;
use crate::models::{GrassType, IrrigationType, SoilType};
use sqlx::PgPool;
use std::collections::HashMap;

fn config(lawn: &[(&'static str, &str)]) -> Config {
    let mut vars = HashMap::from([("DATABASE_PASSWORD", "x".to_string())]);
    for (key, value) in lawn {
        vars.insert(*key, value.to_string());
    }
    from_vars(&|key| vars.get(key).cloned()).unwrap().0
}

// @spec APP-PROF-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn startup_seeds_the_defaults_once_and_then_ignores_lawn_settings(pool: PgPool) {
    crate::ensure_default_profile(&pool, &config(&[]))
        .await
        .unwrap();
    let seeded = queries::get_default_lawn_profile(&pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(seeded.name, "Main Lawn");
    assert_eq!(seeded.grass_type, GrassType::TallFescue);
    assert_eq!(seeded.usda_zone, "7a");

    let changed = config(&[("LAWN_NAME", "Back Forty"), ("LAWN_GRASS_TYPE", "kbg")]);
    crate::ensure_default_profile(&pool, &changed)
        .await
        .unwrap();
    let after = queries::get_default_lawn_profile(&pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((after.id, after.name.as_str()), (seeded.id, "Main Lawn"));
    assert_eq!(after.grass_type, GrassType::TallFescue);
}

// @spec APP-PROF-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn startup_seeds_from_the_lawn_settings_when_no_profile_exists(pool: PgPool) {
    let lawn = config(&[
        ("LAWN_NAME", "Back Forty"),
        ("LAWN_GRASS_TYPE", "kbg"),
        ("LAWN_USDA_ZONE", "6b"),
        ("LAWN_SOIL_TYPE", "Clay"),
        ("LAWN_SIZE_SQFT", "4200"),
        ("LAWN_IRRIGATION_TYPE", "Hose"),
    ]);
    crate::ensure_default_profile(&pool, &lawn).await.unwrap();
    let seeded = queries::get_default_lawn_profile(&pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(seeded.name, "Back Forty");
    assert_eq!(seeded.grass_type, GrassType::KentuckyBluegrass);
    assert_eq!(seeded.usda_zone, "6b");
    assert_eq!(seeded.soil_type, Some(SoilType::Clay));
    assert_eq!(seeded.lawn_size_sqft, Some(4200.0));
    assert_eq!(seeded.irrigation_type, Some(IrrigationType::Hose));
}

// @spec APP-PROF-005, APP-PROF-015
#[sqlx::test(migrations = "src/db/migrations")]
async fn startup_seeds_the_location_with_a_new_profile(pool: PgPool) {
    let located = config(&[("LAWN_LATITUDE", "39.86"), ("LAWN_LONGITUDE", "-75.79")]);
    crate::ensure_default_profile(&pool, &located)
        .await
        .unwrap();
    let location = queries::get_default_lawn_profile(&pool)
        .await
        .unwrap()
        .unwrap()
        .location
        .expect("a seeded location");
    assert_eq!((location.latitude, location.longitude), (39.86, -75.79));
    assert_eq!(location.timezone, "America/New_York");
}

// @spec APP-PROF-015
#[sqlx::test(migrations = "src/db/migrations")]
async fn startup_gives_a_location_to_an_existing_profile_that_has_none_and_then_leaves_it(
    pool: PgPool,
) {
    // An installation that predates the location: a profile, then the lawn's coordinates.
    crate::ensure_default_profile(&pool, &config(&[("LAWN_NAME", "Old Lawn")]))
        .await
        .unwrap();
    let old = config(&[("LAWN_LATITUDE", "39.86"), ("LAWN_LONGITUDE", "-75.79")]);
    crate::ensure_default_profile(&pool, &old).await.unwrap();
    let profile = queries::get_default_lawn_profile(&pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        profile.name, "Old Lawn",
        "nothing else about the profile changes"
    );
    assert_eq!(profile.location.as_ref().unwrap().latitude, 39.86);

    // Once it has one, the settings are ignored.
    let moved = config(&[("LAWN_LATITUDE", "41.88"), ("LAWN_LONGITUDE", "-87.63")]);
    crate::ensure_default_profile(&pool, &moved).await.unwrap();
    let after = queries::get_default_lawn_profile(&pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.location.unwrap().latitude, 39.86);
}

// @spec APP-PROF-016
#[sqlx::test(migrations = "src/db/migrations")]
async fn an_unusable_location_setting_leaves_the_profile_without_one(pool: PgPool) {
    let bad = config(&[("LAWN_LATITUDE", "0"), ("LAWN_LONGITUDE", "0")]);
    crate::ensure_default_profile(&pool, &bad).await.unwrap();
    let profile = queries::get_default_lawn_profile(&pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(profile.location, None);
}
