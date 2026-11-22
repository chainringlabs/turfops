//! Where the lawn is: the one fact the weather record needs.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The contiguous United States, the only place the weather sources cover.
pub const LATITUDE_RANGE: (f64, f64) = (24.5, 49.5);
pub const LONGITUDE_RANGE: (f64, f64) = (-125.0, -66.9);
/// Coordinates are kept to 4 decimal places (about 10 m); the weather grid is about 4 km.
const DECIMAL_PLACES: i32 = 4;

/// A lawn's location. The timezone always follows from the coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LawnLocation {
    pub latitude: f64,
    pub longitude: f64,
    /// What the owner searched for or typed. Never sent to an outside service.
    pub place_name: Option<String>,
    /// IANA zone name of the point.
    pub timezone: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationError {
    /// Not a point in the contiguous United States (or not a number at all).
    OutsideContiguousUs,
}

impl LawnLocation {
    /// The location at a point, rounded for storage, with its timezone.
    // @spec APP-PROF-013, APP-PROF-014
    pub fn at(
        latitude: f64,
        longitude: f64,
        place_name: Option<String>,
    ) -> Result<Self, LocationError> {
        let inside = |value: f64, (low, high): (f64, f64)| value >= low && value <= high;
        if !inside(latitude, LATITUDE_RANGE) || !inside(longitude, LONGITUDE_RANGE) {
            return Err(LocationError::OutsideContiguousUs);
        }
        let (latitude, longitude) = (rounded(latitude), rounded(longitude));
        Ok(Self {
            latitude,
            longitude,
            place_name: place_name
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty()),
            timezone: timezone_at(latitude, longitude),
        })
    }
}

fn rounded(degrees: f64) -> f64 {
    let scale = 10f64.powi(DECIMAL_PLACES);
    (degrees * scale).round() / scale
}

/// The IANA zone of a point, from the zone map compiled into the application. Building
/// the finder unpacks that map, so it is built once.
// @spec APP-PROF-014
fn timezone_at(latitude: f64, longitude: f64) -> String {
    static FINDER: OnceLock<tzf_rs::DefaultFinder> = OnceLock::new();
    FINDER
        .get_or_init(tzf_rs::DefaultFinder::new)
        .get_tz_name(longitude, latitude)
        .to_string()
}
