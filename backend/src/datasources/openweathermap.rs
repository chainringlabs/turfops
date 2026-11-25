use crate::clock::Clock;
use crate::error::{Result, Source, TurfOpsError};
use crate::models::forecast::{ForecastPoint, ForecastProvider, WeatherCondition, WeatherForecast};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::time::Duration;

const API_BASE_URL: &str = "https://api.openweathermap.org/data/2.5";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub struct OpenWeatherMapClient {
    client: reqwest::Client,
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
/// query string, and reqwest's error text includes the URL, so it is stripped here: no
/// caller, log line or response can then leak the key.
fn request_error(e: reqwest::Error) -> TurfOpsError {
    TurfOpsError::unavailable(Source::Forecast, e.without_url())
}

impl OpenWeatherMapClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("failed to build OpenWeatherMap HTTP client");
        Self {
            client,
            base_url: API_BASE_URL.to_string(),
        }
    }

    /// A client aimed at another endpoint (tests).
    #[cfg(test)]
    pub(crate) fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.to_string();
        self
    }

    /// The 5-day, 3-hour forecast for a point, asked with `key`; daily summaries are
    /// `clock`'s local days.
    // @spec WX-SRC-OWM-001, WX-SRC-FC-010, WX-SRC-CFG-003
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

        let response = self.client.get(&url).send().await.map_err(request_error)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            // @spec WX-SRC-OWM-006
            return Err(TurfOpsError::unavailable(
                Source::Forecast,
                format!("OpenWeatherMap returned {}: {}", status, body),
            ));
        }

        let owm_response: OwmForecastResponse = response.json().await.map_err(|e| {
            TurfOpsError::unavailable(
                Source::Forecast,
                format!(
                    "Failed to parse OpenWeatherMap response: {}",
                    e.without_url()
                ),
            )
        })?;

        let hourly = owm_response
            .list
            .iter()
            .map(convert_forecast_item)
            .collect();
        Ok(WeatherForecast::new(
            ForecastProvider::OpenWeatherMap,
            now,
            hourly,
            clock,
        ))
    }
}

impl Default for OpenWeatherMapClient {
    fn default() -> Self {
        Self::new()
    }
}

/// OpenWeatherMap's slots are three hours long.
const SLOT_HOURS: u32 = 3;

// @spec WX-SRC-FC-002, WX-SRC-OWM-001
fn convert_forecast_item(item: &OwmForecastItem) -> ForecastPoint {
    let weather_condition = item
        .weather
        .first()
        .map(|w| WeatherCondition::from_owm_id(w.id))
        .unwrap_or_default();
    // A slot with no rain or snow entry is a forecast of none.
    let rain_mm = item.rain.as_ref().map(|r| r.three_hour).unwrap_or(0.0);
    let snow_mm = item.snow.as_ref().map(|s| s.three_hour).unwrap_or(0.0);
    ForecastPoint {
        timestamp: DateTime::from_timestamp(item.dt, 0).unwrap_or_else(Utc::now),
        slot_hours: SLOT_HOURS,
        temp_f: item.main.temp,
        feels_like_f: Some(item.main.feels_like),
        humidity_percent: item.main.humidity,
        precipitation_mm: Some(rain_mm + snow_mm),
        precipitation_prob: Some(item.pop),
        wind_speed_mph: item.wind.speed,
        wind_gust_mph: item.wind.gust,
        cloud_cover_percent: Some(item.clouds.all),
        weather_condition,
    }
}

#[cfg(test)]
mod fetch_tests;
#[cfg(test)]
mod tests;
