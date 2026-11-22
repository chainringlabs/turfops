//! Parsers for the USDA AWDB REST API's SCAN station list and hourly data.

use crate::models::environmental::celsius_to_fahrenheit;
use crate::models::weather_record::{HourlyRow, Input, Network, Station};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde::Deserialize;
use std::collections::BTreeMap;

/// SCAN's standard sensor depths in inches, in the record's depth order (5 … 100 cm).
pub const DEPTHS_IN: [i32; 5] = [-2, -4, -8, -20, -40];
/// The list carries stations outside the contiguous United States too.
const NOT_CONTIGUOUS: [&str; 6] = ["AK", "HI", "PR", "VI", "GU", "AS"];
/// An end date this far out means the station is still reporting.
const OPEN_ENDED_YEAR: i32 = 2090;
const MM_PER_INCH: f64 = 25.4;
const TEMPERATURE_F: (f64, f64) = (-80.0, 150.0);
const HOURLY_PRECIP_MM: (f64, f64) = (0.0, 300.0);
const HUMIDITY_PCT: (f64, f64) = (0.0, 100.0);
const MOISTURE: (f64, f64) = (0.0, 1.0);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedStation {
    station_triplet: String,
    station_id: String,
    state_code: String,
    network_code: String,
    name: String,
    elevation: Option<f64>,
    latitude: f64,
    longitude: f64,
    data_time_zone: Option<f64>,
    begin_date: Option<String>,
    end_date: Option<String>,
    #[serde(default)]
    station_elements: Vec<Element>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Element {
    element_code: String,
    height_depth: Option<i32>,
    duration_name: Option<String>,
    stored_unit_code: Option<String>,
}

fn depth_index(depth_in: Option<i32>) -> Option<usize> {
    DEPTHS_IN.iter().position(|d| Some(*d) == depth_in)
}

/// The record input an hourly element supplies, if any.
fn input_of(element: &Element) -> Option<Input> {
    const SOIL_TEMP: [Input; 5] = [
        Input::SoilTemp5,
        Input::SoilTemp10,
        Input::SoilTemp20,
        Input::SoilTemp50,
        Input::SoilTemp100,
    ];
    const SOIL_MOISTURE: [Input; 5] = [
        Input::SoilMoisture5,
        Input::SoilMoisture10,
        Input::SoilMoisture20,
        Input::SoilMoisture50,
        Input::SoilMoisture100,
    ];
    match element.element_code.as_str() {
        "TOBS" => Some(Input::AirTemp),
        "RHUM" => Some(Input::Humidity),
        "PRCP" => Some(Input::Rain),
        "STO" => depth_index(element.height_depth).map(|i| SOIL_TEMP[i]),
        "SMS" => depth_index(element.height_depth).map(|i| SOIL_MOISTURE[i]),
        _ => None,
    }
}

fn year_of(date: &Option<String>) -> Option<i32> {
    date.as_deref()?.get(..4)?.parse().ok()
}

/// The SCAN stations of the contiguous United States, each with the hourly inputs it reports.
// @spec WX-SRC-SCAN-001
pub fn catalog(json: &str) -> Result<Vec<Station>, String> {
    let listed: Vec<ListedStation> =
        serde_json::from_str(json).map_err(|e| format!("SCAN station list did not parse: {e}"))?;
    Ok(listed
        .into_iter()
        .filter(|s| s.network_code == "SCAN" && !NOT_CONTIGUOUS.contains(&s.state_code.as_str()))
        .map(|s| {
            let mut inputs: Vec<Input> = s
                .station_elements
                .iter()
                .filter(|e| e.duration_name.as_deref() == Some("HOURLY"))
                .filter_map(input_of)
                .collect();
            inputs.sort_by_key(|input| Input::ALL.iter().position(|i| i == input));
            inputs.dedup();
            Station {
                network: Network::Scan,
                first_year: year_of(&s.begin_date),
                last_year: year_of(&s.end_date).filter(|year| *year < OPEN_ENDED_YEAR),
                station_id: s.station_id,
                name: s.name,
                state: s.state_code,
                latitude: s.latitude,
                longitude: s.longitude,
                elevation_ft: s.elevation,
                inputs,
                locator: Some(s.station_triplet),
                utc_offset_hours: s.data_time_zone,
            }
        })
        .collect())
}

#[derive(Deserialize)]
struct StationData {
    data: Vec<Series>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Series {
    station_element: Element,
    values: Vec<Reading>,
}

#[derive(Deserialize)]
struct Reading {
    date: String,
    value: Option<f64>,
}

fn within(value: f64, (low, high): (f64, f64)) -> Option<f64> {
    (value >= low && value <= high).then_some(value)
}

/// A reading in record units, by the unit the response declares for its element.
// @spec WX-SRC-NORM-001, WX-SRC-NORM-002
fn in_record_units(input: Input, value: f64, unit: &str) -> Option<f64> {
    match input {
        Input::AirTemp
        | Input::SoilTemp5
        | Input::SoilTemp10
        | Input::SoilTemp20
        | Input::SoilTemp50
        | Input::SoilTemp100 => {
            let fahrenheit = match unit {
                "degF" => value,
                "degC" => celsius_to_fahrenheit(value),
                _ => return None,
            };
            within(fahrenheit, TEMPERATURE_F)
        }
        Input::Humidity => (unit == "pct")
            .then_some(value)
            .and_then(|v| within(v, HUMIDITY_PCT)),
        Input::Rain => {
            let mm = match unit {
                "in" => value * MM_PER_INCH,
                "mm" => value,
                _ => return None,
            };
            within(mm, HOURLY_PRECIP_MM)
        }
        Input::DailyAir => None,
        // Soil moisture arrives as a percentage by volume.
        _ => (unit == "pct")
            .then_some(value / 100.0)
            .and_then(|v| within(v, MOISTURE)),
    }
}

fn set(row: &mut HourlyRow, input: Input, value: f64) {
    match input {
        Input::AirTemp => row.air_temp_f = Some(value),
        Input::Humidity => row.rh_pct = Some(value),
        Input::Rain => row.precip_mm = Some(value),
        Input::SoilTemp5 => row.soil_temp_f[0] = Some(value),
        Input::SoilTemp10 => row.soil_temp_f[1] = Some(value),
        Input::SoilTemp20 => row.soil_temp_f[2] = Some(value),
        Input::SoilTemp50 => row.soil_temp_f[3] = Some(value),
        Input::SoilTemp100 => row.soil_temp_f[4] = Some(value),
        Input::SoilMoisture5 => row.soil_moisture[0] = Some(value),
        Input::SoilMoisture10 => row.soil_moisture[1] = Some(value),
        Input::SoilMoisture20 => row.soil_moisture[2] = Some(value),
        Input::SoilMoisture50 => row.soil_moisture[3] = Some(value),
        Input::SoilMoisture100 => row.soil_moisture[4] = Some(value),
        Input::DailyAir => {}
    }
}

/// One station's hours from a data response, ascending. Times arrive in the station's
/// fixed local standard time and are converted to UTC with `utc_offset_hours`.
// @spec WX-SRC-SCAN-002, WX-SRC-SCAN-003, WX-SRC-NORM-004
pub fn hourly(json: &str, utc_offset_hours: f64) -> Result<Vec<HourlyRow>, String> {
    let stations: Vec<StationData> =
        serde_json::from_str(json).map_err(|e| format!("SCAN data did not parse: {e}"))?;
    let offset = Duration::minutes((utc_offset_hours * 60.0).round() as i64);
    let mut rows: BTreeMap<DateTime<Utc>, HourlyRow> = BTreeMap::new();
    for series in stations.iter().flat_map(|s| &s.data) {
        let element = &series.station_element;
        let Some(input) = input_of(element) else {
            continue;
        };
        let unit = element.stored_unit_code.as_deref().unwrap_or("");
        for reading in &series.values {
            let Ok(local) = NaiveDateTime::parse_from_str(&reading.date, "%Y-%m-%d %H:%M") else {
                continue;
            };
            let observed_at = (local - offset).and_utc();
            if local.and_utc().timestamp() % 3600 != 0 {
                continue;
            }
            let row = rows.entry(observed_at).or_insert_with(|| HourlyRow {
                observed_at,
                ..Default::default()
            });
            if let Some(value) = reading.value.and_then(|v| in_record_units(input, v, unit)) {
                set(row, input, value);
            }
        }
    }
    Ok(rows.into_values().collect())
}
