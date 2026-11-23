//! The weather record's stored shapes (WX-REC): stations, sources, hourly and daily rows.
//! Every value is in record units: °F, mm, percent relative humidity, soil moisture 0–1.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

macro_rules! names {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The name stored in the database.
            pub fn as_str(&self) -> &'static str {
                match self { $($name::$variant => $text),+ }
            }

            pub fn parse(text: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|v| v.as_str() == text)
            }
        }
    };
}

names! {
    /// A network of measured stations.
    Network { Uscrn => "Uscrn", Scan => "Scan" }
}

names! {
    /// What kind of thing a source is.
    SourceKind {
        Sensor => "Sensor", Station => "Station", Grid => "Grid",
        Analysis => "Analysis", Estimate => "Estimate",
    }
}

names! {
    /// Whether a source can speak for the lawn or only calibrates the soil estimate.
    SourceRole { Basis => "Basis", Calibration => "Calibration" }
}

names! {
    /// One thing the record resolves for the lawn, and one thing a sensor can measure.
    Input {
        AirTemp => "air_temp", Humidity => "humidity", Rain => "rain", DailyAir => "daily_air",
        SoilTemp5 => "soil_temp_5", SoilTemp10 => "soil_temp_10", SoilTemp20 => "soil_temp_20",
        SoilTemp50 => "soil_temp_50", SoilTemp100 => "soil_temp_100",
        SoilMoisture5 => "soil_moisture_5", SoilMoisture10 => "soil_moisture_10",
        SoilMoisture20 => "soil_moisture_20", SoilMoisture50 => "soil_moisture_50",
        SoilMoisture100 => "soil_moisture_100",
    }
}

impl Input {
    /// The input as the owner reads it.
    pub fn label(&self) -> &'static str {
        match self {
            Input::AirTemp => "Air temperature",
            Input::Humidity => "Humidity",
            Input::Rain => "Rain",
            Input::DailyAir => "Daily high and low",
            Input::SoilTemp5 => "Soil temperature, 5 cm",
            Input::SoilTemp10 => "Soil temperature, 10 cm",
            Input::SoilTemp20 => "Soil temperature, 20 cm",
            Input::SoilTemp50 => "Soil temperature, 50 cm",
            Input::SoilTemp100 => "Soil temperature, 100 cm",
            Input::SoilMoisture5 => "Soil moisture, 5 cm",
            Input::SoilMoisture10 => "Soil moisture, 10 cm",
            Input::SoilMoisture20 => "Soil moisture, 20 cm",
            Input::SoilMoisture50 => "Soil moisture, 50 cm",
            Input::SoilMoisture100 => "Soil moisture, 100 cm",
        }
    }
}

/// A station in a network's catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct Station {
    pub network: Network,
    pub station_id: String,
    pub name: String,
    pub state: String,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation_ft: Option<f64>,
    pub inputs: Vec<Input>,
    pub first_year: Option<i32>,
    pub last_year: Option<i32>,
    pub locator: Option<String>,
    pub utc_offset_hours: Option<f64>,
}

/// A source in use for the lawn, as it is to be stored.
#[derive(Debug, Clone, PartialEq)]
pub struct NewSource {
    pub key: String,
    pub kind: SourceKind,
    pub name: String,
    /// The catalog row of a station source.
    pub station_ref: Option<i64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub distance_miles: Option<f64>,
    pub role: SourceRole,
}

/// A stored source.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub id: i64,
    pub key: String,
    pub kind: SourceKind,
    pub name: String,
    pub station_ref: Option<i64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub distance_miles: Option<f64>,
    pub role: SourceRole,
}

impl Source {
    /// Whether this source publishes daily values only, and so may hold daily rows: the
    /// grids, the soil estimate, and stations that only calibrate it.
    // @spec WX-REC-STORE-005
    pub fn publishes_daily(&self) -> bool {
        match self.kind {
            SourceKind::Grid | SourceKind::Estimate => true,
            SourceKind::Station => self.role == SourceRole::Calibration,
            SourceKind::Sensor | SourceKind::Analysis => false,
        }
    }
}

/// One source's values for one UTC hour, stamped at the start of the hour.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HourlyRow {
    pub observed_at: DateTime<Utc>,
    pub air_temp_f: Option<f64>,
    pub air_temp_max_f: Option<f64>,
    pub air_temp_min_f: Option<f64>,
    pub rh_pct: Option<f64>,
    pub precip_mm: Option<f64>,
    /// Soil temperature at 5, 10, 20, 50 and 100 cm.
    pub soil_temp_f: [Option<f64>; 5],
    /// Soil moisture at the same depths.
    pub soil_moisture: [Option<f64>; 5],
}

/// One source's values for one of its own days.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DailyRow {
    pub day: NaiveDate,
    pub air_max_f: Option<f64>,
    pub air_min_f: Option<f64>,
    pub air_mean_f: Option<f64>,
    pub precip_mm: Option<f64>,
    pub soil_temp_5_f: Option<f64>,
    pub soil_temp_10_f: Option<f64>,
}

/// What a source's product is wanted for, what is held, and how its fetches have gone.
#[derive(Debug, Clone, PartialEq)]
pub struct FetchState {
    pub source_id: i64,
    pub product: String,
    pub wanted_from: NaiveDate,
    /// First and last day held, or `None` when nothing is.
    pub held: Option<(NaiveDate, NaiveDate)>,
    pub last_attempt: Option<DateTime<Utc>>,
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}
