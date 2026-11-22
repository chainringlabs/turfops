//! Distance on the earth's surface.

/// Mean earth radius in statute miles.
const EARTH_RADIUS_MILES: f64 = 3958.7613;

/// Great-circle distance in miles between two points given in degrees.
// @spec WX-REC-PLAN-009
pub fn miles_between(lat_a: f64, lon_a: f64, lat_b: f64, lon_b: f64) -> f64 {
    let (phi_a, phi_b) = (lat_a.to_radians(), lat_b.to_radians());
    let d_phi = (lat_b - lat_a).to_radians();
    let d_lambda = (lon_b - lon_a).to_radians();
    let h =
        (d_phi / 2.0).sin().powi(2) + phi_a.cos() * phi_b.cos() * (d_lambda / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_MILES * h.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    // @spec WX-REC-PLAN-009
    #[test]
    fn distance_is_great_circle_miles() {
        // Philadelphia to Pittsburgh city centres: about 257 miles.
        let miles = miles_between(39.9526, -75.1652, 40.4406, -79.9959);
        assert!((miles - 257.0).abs() < 2.0, "{miles}");
        assert_eq!(miles_between(40.0, -75.0, 40.0, -75.0), 0.0);
        // One degree of latitude is about 69.1 miles anywhere.
        assert!((miles_between(30.0, -90.0, 31.0, -90.0) - 69.09).abs() < 0.1);
    }
}
