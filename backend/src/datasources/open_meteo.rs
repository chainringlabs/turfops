//! Open-Meteo (WX-SRC-OM): the hourly analysis of past air temperature, humidity and
//! precipitation at a point, the point's elevation, the forecast, and the place search
//! used in setup.
//! Soil variables are never asked for.

mod forecast;
mod places;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use places::Place;

use super::outside::{Fetched, Outside};
use crate::error::{Result, Source, TurfOpsError};
use crate::models::weather_record::HourlyRow;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use std::sync::Arc;

const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
const SEARCH_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
/// The furthest back the forecast API serves past hours.
const MAX_DAYS_BACK: u32 = 92;
const FEET_PER_METRE: f64 = 3.28084;
const MM_PER_INCH: f64 = 25.4;
pub(super) const TEMPERATURE_F: (f64, f64) = (-80.0, 150.0);
pub(super) const HUMIDITY_PCT: (f64, f64) = (0.0, 100.0);
pub(super) const HOURLY_PRECIP_MM: (f64, f64) = (0.0, 300.0);

/// A point's past hours, and the elevation of the model grid they are for.
#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    pub elevation_ft: Option<f64>,
    pub hours: Vec<HourlyRow>,
}

#[derive(Deserialize)]
struct Body {
    elevation: Option<f64>,
    hourly_units: Units,
    hourly: Hourly,
}

#[derive(Deserialize)]
struct Units {
    temperature_2m: String,
    precipitation: String,
}

#[derive(Deserialize)]
struct Hourly {
    time: Vec<i64>,
    temperature_2m: Vec<Option<f64>>,
    relative_humidity_2m: Vec<Option<f64>>,
    precipitation: Vec<Option<f64>>,
}

pub struct OpenMeteoClient {
    outside: Arc<Outside>,
    forecast_url: String,
    search_url: String,
}

impl OpenMeteoClient {
    pub fn new(outside: Arc<Outside>) -> Self {
        Self {
            outside,
            forecast_url: FORECAST_URL.into(),
            search_url: SEARCH_URL.into(),
        }
    }

    #[cfg(test)]
    pub fn with_urls(mut self, forecast_url: &str, search_url: &str) -> Self {
        self.forecast_url = forecast_url.into();
        self.search_url = search_url.into();
        self
    }

    /// The hours that have ended by `now`, from `days_back` days ago (92 at most).
    // @spec WX-SRC-OM-001, WX-SRC-OM-002, WX-SRC-OM-003, WX-SRC-REQ-006, WX-SRC-ERR-001
    pub async fn analysis(
        &self,
        latitude: f64,
        longitude: f64,
        days_back: u32,
        now: DateTime<Utc>,
    ) -> Result<Analysis> {
        // Two forecast days, so the newest past hour's rain (reported at its end) is in
        // the response even when that end falls on the next UTC day.
        let url = format!(
            "{}?latitude={latitude}&longitude={longitude}\
             &hourly=temperature_2m,relative_humidity_2m,precipitation\
             &past_days={}&forecast_days=2&temperature_unit=fahrenheit\
             &timeformat=unixtime&timezone=GMT",
            self.forecast_url,
            days_back.clamp(1, MAX_DAYS_BACK),
        );
        let text = self.body(&url).await.map_err(analysis_down)?;
        parse(&text, now).map_err(analysis_down)
    }

    /// The body of a GET that must return one.
    pub(super) async fn body(&self, url: &str) -> std::result::Result<String, String> {
        match self.outside.get(url, None).await? {
            Fetched::Body { text, .. } => Ok(text),
            Fetched::NotModified | Fetched::Missing => Err(format!("{url} returned no body")),
        }
    }
}

fn analysis_down(detail: String) -> TurfOpsError {
    TurfOpsError::unavailable(Source::HourlyAnalysis, detail)
}

pub(super) fn within(value: f64, (low, high): (f64, f64)) -> Option<f64> {
    (value >= low && value <= high).then_some(value)
}

/// The past hours of an analysis response. Temperature and humidity are the model's
/// values at the start of the hour; precipitation is reported as the sum over the hour
/// before its timestamp, so an hour's rain is the amount stamped at its end.
// @spec WX-SRC-OM-001, WX-SRC-OM-003, WX-SRC-NORM-001, WX-SRC-NORM-002, WX-SRC-NORM-004
pub fn parse(json: &str, now: DateTime<Utc>) -> std::result::Result<Analysis, String> {
    let body: Body =
        serde_json::from_str(json).map_err(|e| format!("analysis did not parse: {e}"))?;
    let Hourly {
        time,
        temperature_2m: temperature,
        relative_humidity_2m: humidity,
        precipitation,
    } = body.hourly;
    if [temperature.len(), humidity.len(), precipitation.len()] != [time.len(); 3] {
        return Err("analysis series differ in length".into());
    }
    let to_f = temperature_scale(&body.hourly_units.temperature_2m)?;
    let to_mm = precipitation_scale(&body.hourly_units.precipitation)?;
    let rain_ending: std::collections::HashMap<i64, f64> = time
        .iter()
        .zip(&precipitation)
        .filter_map(|(end, amount)| Some((*end, (*amount)? * to_mm)))
        .collect();
    let mut hours = Vec::with_capacity(time.len());
    for (index, start) in time.iter().enumerate() {
        let observed_at = DateTime::from_timestamp(*start, 0)
            .ok_or_else(|| format!("analysis time {start} is not a time"))?;
        let end = observed_at + Duration::hours(1);
        if end > now {
            continue;
        }
        let row = HourlyRow {
            observed_at,
            air_temp_f: temperature[index]
                .map(to_f)
                .and_then(|v| within(v, TEMPERATURE_F)),
            rh_pct: humidity[index].and_then(|v| within(v, HUMIDITY_PCT)),
            precip_mm: rain_ending
                .get(&end.timestamp())
                .and_then(|v| within(*v, HOURLY_PRECIP_MM)),
            ..Default::default()
        };
        if row.air_temp_f.is_some() || row.rh_pct.is_some() || row.precip_mm.is_some() {
            hours.push(row);
        }
    }
    Ok(Analysis {
        elevation_ft: body
            .elevation
            .filter(|metres| metres.is_finite())
            .map(|metres| metres * FEET_PER_METRE),
        hours,
    })
}

pub(super) fn temperature_scale(unit: &str) -> std::result::Result<fn(f64) -> f64, String> {
    match unit {
        "°F" => Ok(|f| f),
        "°C" => Ok(|c| c * 9.0 / 5.0 + 32.0),
        other => Err(format!("temperature unit {other:?} is not known")),
    }
}

pub(super) fn precipitation_scale(unit: &str) -> std::result::Result<f64, String> {
    match unit {
        "mm" => Ok(1.0),
        "inch" => Ok(MM_PER_INCH),
        other => Err(format!(
            "analysis precipitation unit {other:?} is not known"
        )),
    }
}
