//! Parsers for the two public inputs: USCRN `daily01` station-year files and the
//! RCC-ACIS grid's daily maximum and minimum at a point.

use crate::models::environmental::celsius_to_fahrenheit;
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// `daily01` values at or below this are missing markers (−99, −9999).
const STATION_MISSING_BELOW: f64 = -90.0;
/// Grid values at or below this are the missing marker (−999).
const GRID_MISSING_BELOW: f64 = -900.0;
/// 0-based fields of a `daily01` record.
const FIELD_DATE: usize = 1;
const FIELD_LONGITUDE: usize = 3;
const FIELD_LATITUDE: usize = 4;
const FIELD_SOIL_5: usize = 23;
const FIELD_SOIL_10: usize = 24;
const FIELD_COUNT: usize = 28;

/// One station's measured daily soil temperatures (°F) and where it is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StationSoil {
    pub latitude: f64,
    pub longitude: f64,
    pub soil_5cm_f: BTreeMap<NaiveDate, f64>,
    pub soil_10cm_f: BTreeMap<NaiveDate, f64>,
}

/// Adds one `daily01` file's days to `station`. Missing markers (−9999, −99) and lines
/// that do not parse are skipped.
// @spec WX-EST-SCORE-001
pub fn parse_daily01(text: &str, station: &mut StationSoil) {
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < FIELD_COUNT {
            continue;
        }
        let Ok(day) = NaiveDate::parse_from_str(fields[FIELD_DATE], "%Y%m%d") else {
            continue;
        };
        // Coordinates are plain numbers: a longitude west of 90° W is not a missing marker.
        if let (Ok(lat), Ok(lon)) = (
            fields[FIELD_LATITUDE].parse::<f64>(),
            fields[FIELD_LONGITUDE].parse::<f64>(),
        ) {
            station.latitude = lat;
            station.longitude = lon;
        }
        if let Some(celsius) = number(fields[FIELD_SOIL_5]) {
            station
                .soil_5cm_f
                .insert(day, celsius_to_fahrenheit(celsius));
        }
        if let Some(celsius) = number(fields[FIELD_SOIL_10]) {
            station
                .soil_10cm_f
                .insert(day, celsius_to_fahrenheit(celsius));
        }
    }
}

fn number(field: &str) -> Option<f64> {
    field
        .parse::<f64>()
        .ok()
        .filter(|value| *value > STATION_MISSING_BELOW)
}

#[derive(Deserialize)]
struct GridBody {
    data: Vec<(String, Value, Value)>,
}

/// Daily air (°F), half the sum of maximum and minimum, from a grid response
/// `{"data": [["YYYY-MM-DD", max, min], …]}`. Days with a missing marker are left out.
// @spec WX-EST-SCORE-001
pub fn parse_grid(json: &str) -> Result<BTreeMap<NaiveDate, f64>, serde_json::Error> {
    let body: GridBody = serde_json::from_str(json)?;
    let present = |value: &Value| value.as_f64().filter(|v| *v > GRID_MISSING_BELOW);
    Ok(body
        .data
        .iter()
        .filter_map(|(day, max, min)| {
            let day = NaiveDate::parse_from_str(day, "%Y-%m-%d").ok()?;
            Some((day, (present(max)? + present(min)?) / 2.0))
        })
        .collect())
}
