//! Parsers for the USCRN station list and the `hourly02` and `daily01` station-year files.

use crate::models::environmental::celsius_to_fahrenheit;
use crate::models::weather_record::{DailyRow, HourlyRow, Input, Network, Station};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};

/// Values at or below this are the files' missing markers (−99, −9999).
const MISSING_BELOW: f64 = -90.0;
/// The reference network and its regional networks publish the same products.
const NETWORKS: [&str; 3] = ["USCRN", "USRCRN", "Alabama-USRCRN"];
const NOT_CONTIGUOUS: [&str; 2] = ["AK", "HI"];
/// Plausible ranges, in record units.
const TEMPERATURE_F: (f64, f64) = (-80.0, 150.0);
const HOURLY_PRECIP_MM: (f64, f64) = (0.0, 300.0);
const DAILY_PRECIP_MM: (f64, f64) = (0.0, 1500.0);
const HUMIDITY_PCT: (f64, f64) = (0.0, 100.0);
const MOISTURE: (f64, f64) = (0.0, 1.0);

fn within(value: f64, (low, high): (f64, f64)) -> Option<f64> {
    (value >= low && value <= high).then_some(value)
}

fn number(field: &str) -> Option<f64> {
    field.parse::<f64>().ok().filter(|v| *v > MISSING_BELOW)
}

fn fahrenheit(field: &str) -> Option<f64> {
    number(field)
        .map(celsius_to_fahrenheit)
        .and_then(|f| within(f, TEMPERATURE_F))
}

/// The operating stations of the contiguous United States from `stations.tsv`.
// @spec WX-SRC-USCRN-001
pub fn catalog(tsv: &str) -> Vec<Station> {
    let mut lines = tsv.lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let columns: Vec<&str> = header.split('\t').collect();
    let at = |name: &str| columns.iter().position(|c| *c == name);
    let (Some(wban), Some(country), Some(state), Some(location), Some(vector), Some(name)) = (
        at("WBAN"),
        at("COUNTRY"),
        at("STATE"),
        at("LOCATION"),
        at("VECTOR"),
        at("NAME"),
    ) else {
        return Vec::new();
    };
    let (Some(latitude), Some(longitude), Some(network), Some(operation)) = (
        at("LATITUDE"),
        at("LONGITUDE"),
        at("NETWORK"),
        at("OPERATION"),
    ) else {
        return Vec::new();
    };
    let (elevation, commissioning) = (at("ELEVATION"), at("COMMISSIONING"));
    lines
        .filter_map(|line| {
            let field: Vec<&str> = line.split('\t').collect();
            let get = |index: usize| field.get(index).map(|f| f.trim());
            let kept = NETWORKS.contains(&get(network)?)
                && get(country)? == "US"
                && !NOT_CONTIGUOUS.contains(&get(state)?)
                && get(operation)? == "Operational";
            if !kept {
                return None;
            }
            let place = format!("{} {}", get(location)?, get(vector)?);
            Some(Station {
                network: Network::Uscrn,
                station_id: get(wban)?.to_string(),
                name: format!("{place} ({})", get(name)?),
                state: get(state)?.to_string(),
                latitude: get(latitude)?.parse().ok()?,
                longitude: get(longitude)?.parse().ok()?,
                elevation_ft: elevation.and_then(get).and_then(|e| e.parse().ok()),
                // What a station reports is learned from its data.
                inputs: Vec::new(),
                first_year: commissioning
                    .and_then(get)
                    .and_then(|c| c.get(..4)?.parse().ok()),
                last_year: None,
                locator: Some(format!("{}_{place}", get(state)?).replace(' ', "_")),
                utc_offset_hours: None,
            })
        })
        .collect()
}

/// How many of a file's non-blank lines gave no row, when `read` rows came from it.
pub fn unreadable_lines(text: &str, read: usize) -> usize {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .count()
        .saturating_sub(read)
}

/// 0-based fields of an `hourly02` record.
mod hourly {
    pub const UTC_DATE: usize = 1;
    pub const UTC_TIME: usize = 2;
    pub const AIR_AVG: usize = 9;
    pub const AIR_MAX: usize = 10;
    pub const AIR_MIN: usize = 11;
    pub const PRECIP: usize = 12;
    pub const HUMIDITY: usize = 26;
    pub const HUMIDITY_FLAG: usize = 27;
    pub const MOISTURE_5: usize = 28;
    pub const SOIL_5: usize = 33;
    pub const COUNT: usize = 38;
}

/// The hours of one `hourly02` file. Each is stamped at the start of the hour it
/// describes: the file's UTC time marks the hour's end.
// @spec WX-SRC-USCRN-002, WX-SRC-USCRN-003, WX-SRC-USCRN-004, WX-SRC-NORM-001, WX-SRC-NORM-002, WX-SRC-NORM-003, WX-SRC-NORM-004
pub fn hourly02(text: &str) -> Vec<HourlyRow> {
    text.lines().filter_map(hourly_row).collect()
}

fn hourly_row(line: &str) -> Option<HourlyRow> {
    use hourly::*;
    let field: Vec<&str> = line.split_whitespace().collect();
    if field.len() < COUNT {
        return None;
    }
    let stamp = format!("{}{}", field[UTC_DATE], field[UTC_TIME]);
    let hour_end = NaiveDateTime::parse_from_str(&stamp, "%Y%m%d%H%M").ok()?;
    let observed_at: DateTime<Utc> = (hour_end - Duration::hours(1)).and_utc();
    let humidity_good = field[HUMIDITY_FLAG] == "0";
    let depths = |first: usize, read: &dyn Fn(&str) -> Option<f64>| -> [Option<f64>; 5] {
        std::array::from_fn(|depth| read(field[first + depth]))
    };
    Some(HourlyRow {
        observed_at,
        air_temp_f: fahrenheit(field[AIR_AVG]),
        air_temp_max_f: fahrenheit(field[AIR_MAX]),
        air_temp_min_f: fahrenheit(field[AIR_MIN]),
        rh_pct: number(field[HUMIDITY])
            .filter(|_| humidity_good)
            .and_then(|v| within(v, HUMIDITY_PCT)),
        precip_mm: number(field[PRECIP]).and_then(|v| within(v, HOURLY_PRECIP_MM)),
        soil_temp_f: depths(SOIL_5, &fahrenheit),
        soil_moisture: depths(MOISTURE_5, &|f| number(f).and_then(|v| within(v, MOISTURE))),
    })
}

/// 0-based fields of a `daily01` record.
mod daily {
    pub const DATE: usize = 1;
    pub const AIR_MAX: usize = 5;
    pub const AIR_MIN: usize = 6;
    pub const AIR_MEAN: usize = 7;
    pub const PRECIP: usize = 9;
    pub const SOIL_5: usize = 23;
    pub const SOIL_10: usize = 24;
    pub const COUNT: usize = 28;
}

/// The days of one `daily01` file, on the station's own local-standard-time day.
// @spec WX-SRC-USCRN-005, WX-SRC-NORM-001, WX-SRC-NORM-002, WX-SRC-NORM-004
pub fn daily01(text: &str) -> Vec<DailyRow> {
    use daily::*;
    text.lines()
        .filter_map(|line| {
            let field: Vec<&str> = line.split_whitespace().collect();
            if field.len() < COUNT {
                return None;
            }
            Some(DailyRow {
                day: NaiveDate::parse_from_str(field[DATE], "%Y%m%d").ok()?,
                air_max_f: fahrenheit(field[AIR_MAX]),
                air_min_f: fahrenheit(field[AIR_MIN]),
                air_mean_f: fahrenheit(field[AIR_MEAN]),
                precip_mm: number(field[PRECIP]).and_then(|v| within(v, DAILY_PRECIP_MM)),
                soil_temp_5_f: fahrenheit(field[SOIL_5]),
                soil_temp_10_f: fahrenheit(field[SOIL_10]),
            })
        })
        .collect()
}

/// The inputs a station's hourly rows show it reports.
// @spec WX-SRC-USCRN-001
pub fn inputs_reported(rows: &[HourlyRow]) -> Vec<Input> {
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
    let any = |pick: &dyn Fn(&HourlyRow) -> Option<f64>| rows.iter().any(|r| pick(r).is_some());
    let mut inputs = Vec::new();
    if any(&|r| r.air_temp_f) {
        inputs.push(Input::AirTemp);
    }
    if any(&|r| r.rh_pct) {
        inputs.push(Input::Humidity);
    }
    if any(&|r| r.precip_mm) {
        inputs.push(Input::Rain);
    }
    for depth in 0..5 {
        if any(&|r| r.soil_temp_f[depth]) {
            inputs.push(SOIL_TEMP[depth]);
        }
        if any(&|r| r.soil_moisture[depth]) {
            inputs.push(SOIL_MOISTURE[depth]);
        }
    }
    inputs
}
