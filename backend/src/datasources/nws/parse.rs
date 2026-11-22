//! A grid-cell forecast as hourly points. Each quantity is a list of values, each valid
//! for a span written as a start and an ISO 8601 duration.

use crate::models::forecast::{ForecastPoint, WeatherCondition};
use chrono::{DateTime, Duration, Timelike, Utc};
use serde::Deserialize;
use std::collections::BTreeMap;

const MPH_PER_KMH: f64 = 0.621_371;
const MPH_PER_MS: f64 = 2.236_94;
/// Longer than any span the service gives; a longer one is not believed.
const MAX_SPAN_HOURS: i64 = 24 * 10;
const CLOUDY_FROM_PCT: f64 = 50.0;
const TEMPERATURE_F: (f64, f64) = (-80.0, 150.0);
const HOURLY_PRECIP_MM: (f64, f64) = (0.0, 300.0);
const PERCENT: (f64, f64) = (0.0, 100.0);

#[derive(Deserialize)]
struct Body {
    properties: Layers,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Layers {
    temperature: Layer,
    relative_humidity: Layer,
    wind_speed: Layer,
    #[serde(default)]
    apparent_temperature: Layer,
    #[serde(default)]
    quantitative_precipitation: Layer,
    #[serde(default)]
    probability_of_precipitation: Layer,
    #[serde(default)]
    wind_gust: Layer,
    #[serde(default)]
    sky_cover: Layer,
    #[serde(default)]
    weather: WeatherLayer,
}

#[derive(Deserialize, Default)]
struct Layer {
    uom: Option<String>,
    #[serde(default)]
    values: Vec<Span<Option<f64>>>,
}

#[derive(Deserialize, Default)]
struct WeatherLayer {
    #[serde(default)]
    values: Vec<Span<Vec<Weather>>>,
}

#[derive(Deserialize)]
struct Span<T> {
    #[serde(rename = "validTime")]
    valid_time: String,
    value: T,
}

#[derive(Deserialize)]
struct Weather {
    coverage: Option<String>,
    weather: Option<String>,
}

type Hours<T> = BTreeMap<DateTime<Utc>, T>;

/// Every quantity of a grid forecast, by hour.
struct Series {
    temperature: Hours<f64>,
    humidity: Hours<f64>,
    wind: Hours<f64>,
    feels_like: Hours<f64>,
    amount: Hours<f64>,
    probability: Hours<f64>,
    gust: Hours<f64>,
    sky: Hours<f64>,
    weather: Hours<WeatherCondition>,
}

/// The hourly points of a grid forecast whose hour has not ended by `now`.
// @spec WX-SRC-FC-002, WX-SRC-FC-003, WX-SRC-FC-006, WX-SRC-FC-009, WX-SRC-NORM-001
pub fn parse(json: &str, now: DateTime<Utc>) -> Result<Vec<ForecastPoint>, String> {
    let layers = serde_json::from_str::<Body>(json)
        .map_err(|e| format!("NWS forecast did not parse: {e}"))?
        .properties;
    let series = Series::of(&layers)?;
    Ok(series
        .temperature
        .keys()
        .filter(|start| **start + Duration::hours(1) > now)
        .filter_map(|start| series.point(*start))
        .collect())
}

impl Series {
    fn of(layers: &Layers) -> Result<Self, String> {
        let to_f = |layer: &Layer| temperature_scale(declared(layer));
        let to_mph = |layer: &Layer| wind_scale(declared(layer));
        let amount = &layers.quantitative_precipitation;
        if !matches!(declared(amount), "wmoUnit:mm" | DOCUMENTED) {
            return Err(format!(
                "NWS precipitation unit {:?} is not known",
                amount.uom
            ));
        }
        let feels_like = &layers.apparent_temperature;
        Ok(Self {
            temperature: hourly(&layers.temperature, to_f(&layers.temperature)?, false)?,
            humidity: hourly(&layers.relative_humidity, |v| v, false)?,
            wind: hourly(&layers.wind_speed, to_mph(&layers.wind_speed)?, false)?,
            feels_like: hourly(feels_like, to_f(feels_like)?, false)?,
            amount: hourly(amount, |v| v, true)?,
            probability: hourly(&layers.probability_of_precipitation, |v| v / 100.0, false)?,
            gust: hourly(&layers.wind_gust, to_mph(&layers.wind_gust)?, false)?,
            sky: hourly(&layers.sky_cover, |v| v, false)?,
            weather: conditions(&layers.weather)?,
        })
    }

    /// The point for the hour starting at `start`, when the forecast gives its
    /// temperature, humidity and wind.
    fn point(&self, start: DateTime<Utc>) -> Option<ForecastPoint> {
        let at = |hours: &Hours<f64>| hours.get(&start).copied();
        let cloud_cover_percent = at(&self.sky).and_then(|v| within(v, PERCENT));
        Some(ForecastPoint {
            timestamp: start,
            slot_hours: 1,
            temp_f: at(&self.temperature).and_then(|v| within(v, TEMPERATURE_F))?,
            feels_like_f: at(&self.feels_like),
            humidity_percent: at(&self.humidity).and_then(|v| within(v, PERCENT))?,
            precipitation_mm: at(&self.amount).and_then(|v| within(v, HOURLY_PRECIP_MM)),
            precipitation_prob: at(&self.probability).and_then(|v| within(v, (0.0, 1.0))),
            wind_speed_mph: at(&self.wind).filter(|v| *v >= 0.0)?,
            wind_gust_mph: at(&self.gust).filter(|v| *v >= 0.0),
            cloud_cover_percent,
            weather_condition: self
                .weather
                .get(&start)
                .copied()
                .unwrap_or_else(|| from_sky(cloud_cover_percent)),
        })
    }
}

fn within(value: f64, (low, high): (f64, f64)) -> Option<f64> {
    (value >= low && value <= high).then_some(value)
}

/// A layer's values by hour. A value stands for every hour of its span; with `shared`
/// it is an amount over the span, divided evenly across its hours.
fn hourly(layer: &Layer, convert: impl Fn(f64) -> f64, shared: bool) -> Result<Hours<f64>, String> {
    let mut hours = Hours::new();
    for span in &layer.values {
        let Some(value) = span.value else { continue };
        let (start, length) = valid_time(&span.valid_time)?;
        let each = if shared { value / length as f64 } else { value };
        for hour in 0..length {
            hours.insert(start + Duration::hours(hour), convert(each));
        }
    }
    Ok(hours)
}

/// The weather forecast for each hour where it is more than a chance.
fn conditions(layer: &WeatherLayer) -> Result<Hours<WeatherCondition>, String> {
    let mut hours = Hours::new();
    for span in &layer.values {
        let named = span.value.iter().find_map(|w| {
            let chance = matches!(w.coverage.as_deref(), Some("chance" | "slight_chance"));
            w.weather.as_deref().filter(|_| !chance).map(condition)
        });
        let Some(condition) = named else { continue };
        let (start, length) = valid_time(&span.valid_time)?;
        for hour in 0..length {
            hours.insert(start + Duration::hours(hour), condition);
        }
    }
    Ok(hours)
}

fn condition(weather: &str) -> WeatherCondition {
    match weather {
        w if w.contains("thunder") => WeatherCondition::Thunderstorm,
        w if w.contains("drizzle") => WeatherCondition::Drizzle,
        w if w.contains("rain") => WeatherCondition::Rain,
        w if w.contains("snow") || w.contains("sleet") || w.contains("ice") => {
            WeatherCondition::Snow
        }
        w if w.contains("fog") => WeatherCondition::Fog,
        "haze" | "smoke" => WeatherCondition::Mist,
        _ => WeatherCondition::Other,
    }
}

fn from_sky(cover: Option<f64>) -> WeatherCondition {
    match cover {
        Some(percent) if percent >= CLOUDY_FROM_PCT => WeatherCondition::Clouds,
        Some(_) => WeatherCondition::Clear,
        None => WeatherCondition::Other,
    }
}

/// The start of a span, on the hour, and its length in whole hours.
fn valid_time(text: &str) -> Result<(DateTime<Utc>, i64), String> {
    let bad = || format!("NWS span {text:?} is not a start and a duration");
    let (start, duration) = text.split_once('/').ok_or_else(bad)?;
    let start = DateTime::parse_from_rfc3339(start)
        .map_err(|_| bad())?
        .with_timezone(&Utc);
    let start = start
        .with_minute(0)
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .ok_or_else(bad)?;
    let hours = duration_hours(duration).ok_or_else(bad)?;
    if !(1..=MAX_SPAN_HOURS).contains(&hours) {
        return Err(bad());
    }
    Ok((start, hours))
}

/// Whole hours in an ISO 8601 duration of days and hours (`PT6H`, `P1D`, `P1DT6H`).
pub(super) fn duration_hours(text: &str) -> Option<i64> {
    let rest = text.strip_prefix('P')?;
    let (days, time) = rest.split_once('T').unwrap_or((rest, ""));
    let count = |part: &str, unit: char| -> Option<i64> {
        if part.is_empty() {
            return Some(0);
        }
        part.strip_suffix(unit)?.parse().ok()
    };
    Some(count(days, 'D')? * 24 + count(time, 'H')?)
}

/// The unit a layer declares. A layer with no values has nothing to convert, so it
/// stands as the unit the service documents.
fn declared(layer: &Layer) -> &str {
    match layer.uom.as_deref() {
        Some(unit) => unit,
        None if layer.values.is_empty() => DOCUMENTED,
        None => "",
    }
}

/// Stands for "whatever the service documents" on a layer that carries no values.
const DOCUMENTED: &str = "documented";

fn temperature_scale(unit: &str) -> Result<fn(f64) -> f64, String> {
    match unit {
        "wmoUnit:degC" | DOCUMENTED => Ok(|c| c * 9.0 / 5.0 + 32.0),
        "wmoUnit:degF" => Ok(|f| f),
        other => Err(format!("NWS temperature unit {other:?} is not known")),
    }
}

fn wind_scale(unit: &str) -> Result<impl Fn(f64) -> f64, String> {
    let factor = match unit {
        "wmoUnit:km_h-1" | DOCUMENTED => MPH_PER_KMH,
        "wmoUnit:m_s-1" => MPH_PER_MS,
        other => return Err(format!("NWS wind unit {other:?} is not known")),
    };
    Ok(move |v| v * factor)
}
