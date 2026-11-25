use chrono::NaiveDate;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalData {
    pub range: String,
    pub soil_temp_10_f: Vec<TimeSeriesPoint>,
    pub ambient_temp_f: Vec<TimeSeriesPoint>,
    pub humidity_percent: Vec<TimeSeriesPoint>,
    pub soil_moisture_10: Vec<TimeSeriesPoint>,
    pub precipitation_mm: Vec<TimeSeriesPoint>,
    pub gdd_accumulation: Vec<TimeSeriesPoint>,
    /// What each series rests on over the range, as runs of days on one basis.
    #[serde(default)]
    pub basis: Vec<SeriesSpan>,
}

/// A run of one chart series' days on one basis.
// @spec WX-VIEW-API-002
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesSpan {
    /// The series' field name, such as `soil_temp_10_f`.
    pub series: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub basis: crate::models::weather_basis::Basis,
}
