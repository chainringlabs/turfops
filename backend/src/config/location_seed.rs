//! Seeding the lawn's location from the environment on a first run.

use super::LawnConfig;
use crate::models::LawnLocation;

/// The pair of settings a location was read from, as given.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct LocationSettings {
    pub latitude_var: &'static str,
    pub longitude_var: &'static str,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
}

impl LocationSettings {
    /// `LAWN_LATITUDE` / `LAWN_LONGITUDE` when either is set, otherwise the
    /// `OWM_LATITUDE` / `OWM_LONGITUDE` pair an older installation may carry.
    // @spec APP-PROF-015
    pub fn from_vars(get: &dyn Fn(&str) -> Option<String>) -> Self {
        let set = |key: &str| get(key).filter(|value| !value.trim().is_empty());
        let pair = |latitude_var: &'static str, longitude_var: &'static str| Self {
            latitude_var,
            longitude_var,
            latitude: set(latitude_var),
            longitude: set(longitude_var),
        };
        let lawn = pair("LAWN_LATITUDE", "LAWN_LONGITUDE");
        if lawn.latitude.is_some() || lawn.longitude.is_some() {
            return lawn;
        }
        pair("OWM_LATITUDE", "OWM_LONGITUDE")
    }
}

impl LawnConfig {
    /// The location the settings describe, and a warning when they describe none usable.
    /// Never a default: a location that cannot be read is no location.
    // @spec APP-PROF-015, APP-PROF-016
    pub fn seed_location(&self) -> (Option<LawnLocation>, Vec<String>) {
        let settings = &self.location;
        let (latitude, longitude) = match (&settings.latitude, &settings.longitude) {
            (None, None) => return (None, Vec::new()),
            (Some(_), None) => return (None, vec![missing(settings.longitude_var, settings)]),
            (None, Some(_)) => return (None, vec![missing(settings.latitude_var, settings)]),
            (Some(latitude), Some(longitude)) => (latitude, longitude),
        };
        let number = |var: &str, raw: &str| {
            raw.trim().parse::<f64>().map_err(|_| {
                format!("{var}={raw:?} isn't a number; leaving the lawn's location unset.")
            })
        };
        let parsed = number(settings.latitude_var, latitude)
            .and_then(|lat| Ok((lat, number(settings.longitude_var, longitude)?)));
        match parsed {
            Err(warning) => (None, vec![warning]),
            Ok((lat, lon)) => match LawnLocation::at(lat, lon, None) {
                Ok(location) => (Some(location), Vec::new()),
                Err(_) => (
                    None,
                    vec![format!(
                        "{}={latitude:?} and {}={longitude:?} are outside the contiguous United \
                     States; leaving the lawn's location unset.",
                        settings.latitude_var, settings.longitude_var
                    )],
                ),
            },
        }
    }
}

fn missing(var: &str, settings: &LocationSettings) -> String {
    format!(
        "{var} isn't set, and a location needs both {} and {}; leaving the lawn's location unset.",
        settings.latitude_var, settings.longitude_var
    )
}
