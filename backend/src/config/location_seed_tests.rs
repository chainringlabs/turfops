//! The lawn's location from `LAWN_LATITUDE` / `LAWN_LONGITUDE` (or the older `OWM_` pair).

use super::load::from_vars;
use super::LawnConfig;
use std::collections::HashMap;

fn lawn(vars: &[(&'static str, &str)]) -> LawnConfig {
    let mut all = HashMap::from([("DATABASE_PASSWORD", "x".to_string())]);
    for (key, value) in vars {
        all.insert(*key, value.to_string());
    }
    from_vars(&|key| all.get(key).cloned()).unwrap().0.lawn
}

// @spec APP-PROF-015
#[test]
fn the_lawn_pair_gives_a_location_with_its_timezone() {
    let (location, warnings) =
        lawn(&[("LAWN_LATITUDE", "39.86"), ("LAWN_LONGITUDE", " -75.79 ")]).seed_location();
    let location = location.expect("a location");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!((location.latitude, location.longitude), (39.86, -75.79));
    assert_eq!(location.timezone, "America/New_York");
    assert_eq!(location.place_name, None);
}

// @spec APP-PROF-015
#[test]
fn the_older_forecast_pair_is_used_only_when_the_lawn_pair_is_absent() {
    let owm = [("OWM_LATITUDE", "41.88"), ("OWM_LONGITUDE", "-87.63")];
    let (location, warnings) = lawn(&owm).seed_location();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(location.unwrap().timezone, "America/Chicago");

    let both = [
        owm[0],
        owm[1],
        ("LAWN_LATITUDE", "39.86"),
        ("LAWN_LONGITUDE", "-75.79"),
    ];
    assert_eq!(lawn(&both).seed_location().0.unwrap().latitude, 39.86);
}

// @spec APP-PROF-015
#[test]
fn no_location_settings_is_no_location_and_no_warning() {
    assert!(matches!(lawn(&[]).seed_location(), (None, w) if w.is_empty()));
    // Compose passes unset variables as empty strings.
    let blank = [("LAWN_LATITUDE", ""), ("LAWN_LONGITUDE", "  ")];
    assert!(matches!(lawn(&blank).seed_location(), (None, w) if w.is_empty()));
}

// @spec APP-PROF-016
#[test]
fn half_a_pair_names_the_missing_setting() {
    let (location, warnings) = lawn(&[("LAWN_LATITUDE", "39.86")]).seed_location();
    assert_eq!(location, None);
    assert!(
        warnings[0].contains("LAWN_LONGITUDE isn't set"),
        "{warnings:?}"
    );

    let (location, warnings) = lawn(&[("OWM_LONGITUDE", "-75.79")]).seed_location();
    assert_eq!(location, None);
    assert!(
        warnings[0].contains("OWM_LATITUDE isn't set"),
        "{warnings:?}"
    );
}

// @spec APP-PROF-016
#[test]
fn an_unparseable_value_is_named_with_what_was_given() {
    let (location, warnings) =
        lawn(&[("LAWN_LATITUDE", "39.86N"), ("LAWN_LONGITUDE", "-75.79")]).seed_location();
    assert_eq!(location, None);
    assert!(
        warnings[0].contains("LAWN_LATITUDE") && warnings[0].contains("39.86N"),
        "{warnings:?}"
    );
}

// @spec APP-PROF-016
#[test]
fn a_point_outside_the_contiguous_united_states_is_no_location_never_zero_zero() {
    for (latitude, longitude) in [("0", "0"), ("61.2", "-149.9"), ("-75.79", "39.86")] {
        let (location, warnings) =
            lawn(&[("LAWN_LATITUDE", latitude), ("LAWN_LONGITUDE", longitude)]).seed_location();
        assert_eq!(location, None, "{latitude}, {longitude}");
        assert!(
            warnings[0].contains("LAWN_LATITUDE") && warnings[0].contains("contiguous"),
            "{warnings:?}"
        );
    }
}

// @spec APP-PROF-005, APP-PROF-015
#[test]
fn a_seeded_profile_carries_the_seeded_location() {
    let (profile, warnings) =
        lawn(&[("LAWN_LATITUDE", "39.86"), ("LAWN_LONGITUDE", "-75.79")]).seed_profile();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(profile.location.unwrap().timezone, "America/New_York");
}
