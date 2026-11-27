//! Open-Meteo as a forecast provider: hourly points for 16 days.

#[cfg(test)]
#[path = "forecast_tests.rs"]
mod tests;

use super::{
    precipitation_scale, temperature_scale, within, OpenMeteoClient, HOURLY_PRECIP_MM,
    HUMIDITY_PCT, TEMPERATURE_F,
};
use crate::clock::Clock;
use crate::error::{Result, Source, TurfOpsError};
use crate::models::forecast::{ForecastPoint, ForecastProvider, WeatherCondition, WeatherForecast};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;

const FORECAST_DAYS: u32 = 16;
const MPH_PER_KMH: f64 = 0.621_371;

#[derive(Deserialize)]
struct Body {
    hourly_units: Units,
    hourly: Hourly,
}

#[derive(Deserialize)]
struct Units {
    temperature_2m: String,
    precipitation: String,
    wind_speed_10m: String,
}

type Series = Vec<Option<f64>>;

#[derive(Deserialize)]
struct Hourly {
    time: Vec<i64>,
    temperature_2m: Series,
    relative_humidity_2m: Series,
    apparent_temperature: Series,
    precipitation: Series,
    precipitation_probability: Series,
    wind_speed_10m: Series,
    wind_gusts_10m: Series,
    cloud_cover: Series,
    weather_code: Series,
}

impl OpenMeteoClient {
    /// The forecast for a point, from the hour in progress at `now`.
    // @spec WX-SRC-FC-005, WX-SRC-OM-002, WX-SRC-REQ-006, WX-SRC-ERR-001
    pub async fn forecast(
        &self,
        latitude: f64,
        longitude: f64,
        clock: &Clock,
        now: DateTime<Utc>,
    ) -> Result<WeatherForecast> {
        let url = format!(
            "{}?latitude={latitude}&longitude={longitude}\
             &hourly=temperature_2m,relative_humidity_2m,apparent_temperature,precipitation,\
             precipitation_probability,wind_speed_10m,wind_gusts_10m,cloud_cover,weather_code\
             &forecast_days={FORECAST_DAYS}&temperature_unit=fahrenheit&wind_speed_unit=mph\
             &timeformat=unixtime&timezone=GMT",
            self.forecast_url,
        );
        let text = self.body(&url).await.map_err(down)?;
        let hourly = parse(&text, now).map_err(down)?;
        Ok(WeatherForecast::new(
            ForecastProvider::OpenMeteo,
            now,
            hourly,
            clock,
        ))
    }
}

fn down(detail: String) -> TurfOpsError {
    TurfOpsError::unavailable(Source::Forecast, detail)
}

/// The points of a forecast response. Temperature, humidity, feels-like, wind, cloud
/// cover and the weather code are stamped at an instant; precipitation, its probability
/// and gust cover the hour before their stamp. A point takes the first group from its
/// start and the second from its end.
// @spec WX-SRC-FC-002, WX-SRC-FC-003, WX-SRC-FC-005, WX-SRC-FC-009, WX-SRC-NORM-001
// @spec WX-SRC-NORM-002
pub fn parse(json: &str, now: DateTime<Utc>) -> std::result::Result<Vec<ForecastPoint>, String> {
    let body: Body =
        serde_json::from_str(json).map_err(|e| format!("forecast did not parse: {e}"))?;
    let h = body.hourly;
    let series = [
        &h.temperature_2m,
        &h.relative_humidity_2m,
        &h.apparent_temperature,
        &h.precipitation,
        &h.precipitation_probability,
        &h.wind_speed_10m,
        &h.wind_gusts_10m,
        &h.cloud_cover,
        &h.weather_code,
    ];
    if series.iter().any(|s| s.len() != h.time.len()) {
        return Err("forecast series differ in length".into());
    }
    let to_f = temperature_scale(&body.hourly_units.temperature_2m)?;
    let to_mm = precipitation_scale(&body.hourly_units.precipitation)?;
    let to_mph = match body.hourly_units.wind_speed_10m.as_str() {
        "mp/h" => 1.0,
        "km/h" => MPH_PER_KMH,
        other => return Err(format!("wind unit {other:?} is not known")),
    };
    let scales = Scales {
        to_f,
        to_mm,
        to_mph,
    };
    let mut points = Vec::with_capacity(h.time.len());
    for (start, end) in (0..h.time.len()).zip(1..h.time.len()) {
        let timestamp = DateTime::from_timestamp(h.time[start], 0)
            .ok_or_else(|| format!("forecast time {} is not a time", h.time[start]))?;
        let ends = timestamp + Duration::hours(1);
        if h.time[end] == ends.timestamp() && ends > now {
            points.extend(point(&h, timestamp, start, end, &scales));
        }
    }
    Ok(points)
}

struct Scales {
    to_f: fn(f64) -> f64,
    to_mm: f64,
    to_mph: f64,
}

/// The point for the hour between the stamps at `start` and `end`, when the forecast
/// gives its temperature, humidity and wind.
fn point(
    h: &Hourly,
    timestamp: DateTime<Utc>,
    start: usize,
    end: usize,
    scales: &Scales,
) -> Option<ForecastPoint> {
    Some(ForecastPoint {
        timestamp,
        slot_hours: 1,
        temp_f: h.temperature_2m[start]
            .map(scales.to_f)
            .and_then(|v| within(v, TEMPERATURE_F))?,
        feels_like_f: h.apparent_temperature[start]
            .map(scales.to_f)
            .and_then(|v| within(v, TEMPERATURE_F)),
        humidity_percent: h.relative_humidity_2m[start].and_then(|v| within(v, HUMIDITY_PCT))?,
        precipitation_mm: h.precipitation[end]
            .map(|v| v * scales.to_mm)
            .and_then(|v| within(v, HOURLY_PRECIP_MM)),
        precipitation_prob: h.precipitation_probability[end]
            .map(|percent| percent / 100.0)
            .and_then(|v| within(v, (0.0, 1.0))),
        wind_speed_mph: h.wind_speed_10m[start].filter(|v| *v >= 0.0)? * scales.to_mph,
        wind_gust_mph: h.wind_gusts_10m[end]
            .filter(|v| *v >= 0.0)
            .map(|v| v * scales.to_mph),
        cloud_cover_percent: h.cloud_cover[start].and_then(|v| within(v, (0.0, 100.0))),
        weather_condition: h.weather_code[start]
            .filter(|code| (0.0..=99.0).contains(code))
            .map(|code| WeatherCondition::from_wmo_code(code as u32))
            .unwrap_or(WeatherCondition::Other),
    })
}
