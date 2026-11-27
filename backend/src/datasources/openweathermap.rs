use super::outside::{Fetched, Outside};
use crate::clock::Clock;
use crate::error::{Result, Source, TurfOpsError};
use crate::models::forecast::{ForecastPoint, ForecastProvider, WeatherCondition, WeatherForecast};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;

const API_BASE_URL: &str = "https://api.openweathermap.org/data/2.5";
const TEMPERATURE_F: (f64, f64) = (-80.0, 150.0);
const PERCENT: (f64, f64) = (0.0, 100.0);
/// Up to 300 mm for each hour of a slot.
const SLOT_PRECIP_MM: (f64, f64) = (0.0, 300.0 * SLOT_HOURS as f64);

/// The OpenWeatherMap forecast client. Its requests go through [`Outside`], like every
/// other outside request.
pub struct OpenWeatherMapClient {
    outside: Arc<Outside>,
    base_url: String,
}

// OpenWeatherMap API response structures
#[derive(Debug, Deserialize)]
struct OwmForecastResponse {
    list: Vec<OwmForecastItem>,
}

#[derive(Debug, Deserialize)]
struct OwmForecastItem {
    dt: i64,
    main: OwmMain,
    weather: Vec<OwmWeather>,
    clouds: OwmClouds,
    wind: OwmWind,
    #[serde(default)]
    pop: f64, // probability of precipitation
    #[serde(default)]
    rain: Option<OwmPrecipitation>,
    #[serde(default)]
    snow: Option<OwmPrecipitation>,
}

#[derive(Debug, Deserialize)]
struct OwmMain {
    temp: f64,
    feels_like: f64,
    humidity: f64,
}

#[derive(Debug, Deserialize)]
struct OwmWeather {
    id: u32,
    #[allow(dead_code)]
    main: String,
    #[allow(dead_code)]
    description: String,
}

#[derive(Debug, Deserialize)]
struct OwmClouds {
    all: f64, // cloudiness percentage
}

#[derive(Debug, Deserialize)]
struct OwmWind {
    speed: f64,
    #[serde(default)]
    gust: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct OwmPrecipitation {
    #[serde(rename = "3h", default)]
    three_hour: f64,
}

// @spec WX-SRC-OWM-005, DEPLOY-LOG-003
/// A failed request as a data-source error. The request URL carries the API key in its
/// query string, and a failure's detail can name the URL, so the key is taken out here:
/// no caller, log line or response can then leak it.
fn request_error(detail: &str, key: &str) -> TurfOpsError {
    let detail = if key.is_empty() {
        detail.to_string()
    } else {
        detail.replace(key, "[key]")
    };
    TurfOpsError::unavailable(Source::Forecast, detail)
}

impl OpenWeatherMapClient {
    pub fn new(outside: Arc<Outside>) -> Self {
        Self {
            outside,
            base_url: API_BASE_URL.to_string(),
        }
    }

    /// A client aimed at another endpoint (tests).
    #[cfg(test)]
    pub(crate) fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.to_string();
        self
    }

    /// An unpaced client aimed at `base_url` (tests against a local stub).
    #[cfg(test)]
    pub(crate) fn at(base_url: &str) -> Self {
        Self::new(Arc::new(Outside::unpaced())).with_base_url(base_url)
    }

    /// The 5-day, 3-hour forecast for a point, asked with `key`; daily summaries are
    /// `clock`'s local days.
    // @spec WX-SRC-OWM-001, WX-SRC-FC-010, WX-SRC-CFG-003, WX-SRC-REQ-001, WX-SRC-REQ-002
    // @spec WX-SRC-REQ-003, WX-SRC-OWM-006
    pub async fn forecast(
        &self,
        latitude: f64,
        longitude: f64,
        key: &str,
        clock: &Clock,
        now: DateTime<Utc>,
    ) -> Result<WeatherForecast> {
        let url = format!(
            "{}/forecast?lat={latitude}&lon={longitude}&appid={key}&units=imperial",
            self.base_url
        );
        let text = match self.outside.get(&url, None).await {
            Ok(Fetched::Body { text, .. }) => text,
            Ok(Fetched::NotModified | Fetched::Missing) => {
                return Err(request_error(&format!("{url} returned no forecast"), key))
            }
            Err(detail) => return Err(request_error(&detail, key)),
        };
        let owm_response: OwmForecastResponse = serde_json::from_str(&text).map_err(|e| {
            request_error(
                &format!("Failed to parse OpenWeatherMap response: {e}"),
                key,
            )
        })?;

        let hourly = owm_response
            .list
            .iter()
            .filter_map(convert_forecast_item)
            .collect();
        Ok(WeatherForecast::new(
            ForecastProvider::OpenWeatherMap,
            now,
            hourly,
            clock,
        ))
    }
}

/// OpenWeatherMap's slots are three hours long.
const SLOT_HOURS: u32 = 3;

/// The point for one slot, or none when its time, temperature, humidity or wind speed is
/// missing or implausible; any other implausible value is left empty.
// @spec WX-SRC-FC-002, WX-SRC-FC-003, WX-SRC-OWM-001, WX-SRC-NORM-002, WX-SRC-NORM-004
fn convert_forecast_item(item: &OwmForecastItem) -> Option<ForecastPoint> {
    let weather_condition = item
        .weather
        .first()
        .map(|w| WeatherCondition::from_owm_id(w.id))
        .unwrap_or_default();
    // A slot with no rain or snow entry is a forecast of none.
    let rain_mm = item.rain.as_ref().map(|r| r.three_hour).unwrap_or(0.0);
    let snow_mm = item.snow.as_ref().map(|s| s.three_hour).unwrap_or(0.0);
    Some(ForecastPoint {
        timestamp: DateTime::from_timestamp(item.dt, 0)?,
        slot_hours: SLOT_HOURS,
        temp_f: within(item.main.temp, TEMPERATURE_F)?,
        feels_like_f: within(item.main.feels_like, TEMPERATURE_F),
        humidity_percent: within(item.main.humidity, PERCENT)?,
        precipitation_mm: within(rain_mm + snow_mm, SLOT_PRECIP_MM),
        precipitation_prob: within(item.pop, (0.0, 1.0)),
        wind_speed_mph: Some(item.wind.speed).filter(|v| *v >= 0.0)?,
        wind_gust_mph: item.wind.gust.filter(|v| *v >= 0.0),
        cloud_cover_percent: within(item.clouds.all, PERCENT),
        weather_condition,
    })
}

fn within(value: f64, (low, high): (f64, f64)) -> Option<f64> {
    (value >= low && value <= high).then_some(value)
}

#[cfg(test)]
mod fetch_tests;
#[cfg(test)]
mod norm_tests;
#[cfg(test)]
mod tests;
