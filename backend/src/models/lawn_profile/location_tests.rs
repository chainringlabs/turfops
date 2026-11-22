//! The lawn's location: bounds, stored precision and the timezone that follows from it.

use super::location::*;

// @spec APP-PROF-013
#[test]
fn coordinates_are_stored_to_four_decimal_places() {
    let location = LawnLocation::at(39.861_234_9, -75.786_551, None).unwrap();
    assert_eq!((location.latitude, location.longitude), (39.8612, -75.7866));
}

// @spec APP-PROF-014
#[test]
fn the_timezone_is_the_iana_zone_of_the_point() {
    for (latitude, longitude, zone) in [
        (39.95, -75.17, "America/New_York"),     // Philadelphia
        (41.88, -87.63, "America/Chicago"),      // Chicago
        (39.74, -104.99, "America/Denver"),      // Denver
        (34.05, -118.24, "America/Los_Angeles"), // Los Angeles
        (33.45, -112.07, "America/Phoenix"),     // Phoenix: no daylight saving
        (39.77, -86.16, "America/Indiana/Indianapolis"),
    ] {
        let location = LawnLocation::at(latitude, longitude, None).unwrap();
        assert_eq!(location.timezone, zone, "{latitude}, {longitude}");
    }
}

// @spec APP-PROF-014
#[test]
fn every_timezone_it_returns_is_one_the_clock_can_use() {
    for (latitude, longitude) in [(47.6, -122.3), (25.8, -80.2), (44.9, -68.7), (32.7, -117.2)] {
        let zone = LawnLocation::at(latitude, longitude, None)
            .unwrap()
            .timezone;
        assert!(zone.parse::<chrono_tz::Tz>().is_ok(), "{zone}");
    }
}

// @spec APP-PROF-016, APP-PROF-017
#[test]
fn a_point_outside_the_contiguous_united_states_is_refused() {
    for (latitude, longitude) in [
        (0.0, 0.0),     // never a default
        (61.2, -149.9), // Anchorage
        (21.3, -157.9), // Honolulu
        (51.5, -0.1),   // London
        (24.4, -81.8),  // just south of the range
        (49.6, -100.0), // just north
        (40.0, -125.1), // just west
        (45.0, -66.8),  // just east
        (f64::NAN, -75.0),
    ] {
        assert_eq!(
            LawnLocation::at(latitude, longitude, None),
            Err(LocationError::OutsideContiguousUs),
            "{latitude}, {longitude}"
        );
    }
    assert!(
        LawnLocation::at(LATITUDE_RANGE.0, LONGITUDE_RANGE.1, None).is_ok(),
        "edges are inside"
    );
}

// @spec APP-PROF-013
#[test]
fn the_place_name_is_kept_trimmed_and_an_empty_one_is_none() {
    let named = LawnLocation::at(39.86, -75.79, Some("  Avondale, PA ".into())).unwrap();
    assert_eq!(named.place_name.as_deref(), Some("Avondale, PA"));
    let blank = LawnLocation::at(39.86, -75.79, Some("   ".into())).unwrap();
    assert_eq!(blank.place_name, None);
}
