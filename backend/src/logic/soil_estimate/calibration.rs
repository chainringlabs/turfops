//! Which stations a depth's model is fitted on: the lawn's own basis station where it
//! has enough soil history, otherwise the five nearest USCRN stations that do.

#[cfg(test)]
#[path = "calibration_tests.rs"]
mod tests;

use super::scores::Calibration;
use crate::geo::miles_between;
use crate::logic::weather_record::plan::reports;
use crate::models::weather_record::{Input, Network, Station};
use chrono::NaiveDate;
use std::collections::HashMap;

/// A station needs this many days of soil temperature at a depth to calibrate it.
pub const MIN_SOIL_DAYS: usize = 1000;
/// How many nearest stations are pooled.
pub const NEAREST: usize = 5;
/// Fewer pooled stations than this is too few to fit on.
pub const FEWEST: usize = 3;
/// Before its data is held, a station is taken to have enough when the catalog shows it
/// operating this many years.
const MIN_YEARS_OPERATING: i32 = 3;

/// The first day a model is trained on: USCRN soil probes were network-wide by then.
// @spec WX-EST-CAL-004
pub fn train_from() -> NaiveDate {
    NaiveDate::from_ymd_opt(2012, 1, 1).expect("a real date")
}

/// A station a model may be fitted on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calibrator {
    /// Its row in the catalog.
    pub station_ref: i64,
    pub distance_miles: f64,
}

/// What to do about one depth's model.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Fit on these stations.
    Fit(Calibration, Vec<Calibrator>),
    /// These stations' soil history must be held and counted before anything is fitted.
    Wait(Vec<Calibrator>),
    /// Too few stations qualify: there is no model for this depth.
    NoModel,
}

/// The USCRN stations that may calibrate `depth`, nearest to the point first: those that
/// report it and that the catalog shows operating for at least three years.
// @spec WX-EST-CAL-008
pub fn candidates(
    latitude: f64,
    longitude: f64,
    depth: Input,
    this_year: i32,
    catalog: &[(i64, Station)],
) -> Vec<Calibrator> {
    let mut near: Vec<Calibrator> = catalog
        .iter()
        .filter(|(_, station)| station.network == Network::Uscrn)
        .filter(|(_, station)| reports(&station.inputs, depth))
        .filter(|(_, station)| {
            let last = station.last_year.unwrap_or(this_year);
            station
                .first_year
                .is_some_and(|first| last - first >= MIN_YEARS_OPERATING)
        })
        .map(|(station_ref, station)| Calibrator {
            station_ref: *station_ref,
            distance_miles: miles_between(latitude, longitude, station.latitude, station.longitude),
        })
        .collect();
    near.sort_by(|a, b| a.distance_miles.total_cmp(&b.distance_miles));
    near
}

/// The decision for a depth. `basis` is the station the plan holds for soil at the
/// depth, if any. `soil_days` is how many days of soil at the depth each station is
/// known to hold; a station absent from it has not been counted yet.
// @spec WX-EST-CAL-001, WX-EST-CAL-002, WX-EST-CAL-008, WX-EST-FAIL-001, WX-EST-FAIL-002
pub fn decide(
    basis: Option<Calibrator>,
    candidates: &[Calibrator],
    soil_days: &HashMap<i64, usize>,
) -> Decision {
    let known = |station: &Calibrator| soil_days.get(&station.station_ref).copied();
    if let Some(basis) = basis {
        match known(&basis) {
            Some(days) if days >= MIN_SOIL_DAYS => {
                return Decision::Fit(Calibration::StationAlone, vec![basis]);
            }
            // Its history is still arriving: do not fit on other stations meanwhile
            // and then change the series under the owner.
            None => return Decision::Wait(vec![basis]),
            Some(_) => {}
        }
    }
    let chosen: Vec<Calibrator> = candidates
        .iter()
        .filter(|station| known(station).is_none_or(|days| days >= MIN_SOIL_DAYS))
        .take(NEAREST)
        .copied()
        .collect();
    if chosen.len() < FEWEST {
        Decision::NoModel
    } else if chosen.iter().any(|station| known(station).is_none()) {
        Decision::Wait(chosen)
    } else {
        Decision::Fit(Calibration::NearestFive, chosen)
    }
}
