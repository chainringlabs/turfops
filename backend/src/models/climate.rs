//! The multi-year daily climate record behind the timing windows.

use crate::models::weather_basis::BasisSpan;
use chrono::NaiveDate;

/// One lawn-local day of the climate record. Any field can be missing on a given day.
#[derive(Debug, Clone, PartialEq)]
pub struct ClimateDay {
    pub date: NaiveDate,
    /// Daily mean soil temperature at 5 cm (°F).
    pub soil_temp_5_f: Option<f64>,
    /// The day's 5 cm soil rests on the soil estimate rather than a measurement.
    pub soil_estimated: bool,
    /// Daily minimum air temperature (°F). Drives freeze dates.
    pub air_min_f: Option<f64>,
    pub air_avg_f: Option<f64>,
    pub gdd50: Option<f64>,
}

/// The climate record and what each input in it rests on.
#[derive(Debug, Clone, Default)]
pub struct ClimateRecord {
    /// Ascending by date.
    pub days: Vec<ClimateDay>,
    /// The basis spans of 5 cm soil and daily air.
    pub basis: Vec<BasisSpan>,
}
