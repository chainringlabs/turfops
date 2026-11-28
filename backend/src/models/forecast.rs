mod daily;

use crate::clock::Clock;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// Who a forecast came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForecastProvider {
    OpenMeteo,
    NationalWeatherService,
    OpenWeatherMap,
}

impl ForecastProvider {
    /// The provider's name as a data point's source.
    pub fn label(self) -> &'static str {
        match self {
            ForecastProvider::OpenMeteo => "Open-Meteo",
            ForecastProvider::NationalWeatherService => "National Weather Service",
            ForecastProvider::OpenWeatherMap => "OpenWeatherMap",
        }
    }
}

/// A forecast as every provider returns it: points of the provider's slot length, and
/// the lawn's local days they add up to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherForecast {
    pub fetched_at: DateTime<Utc>,
    // @spec WX-SRC-FC-008
    pub provider: ForecastProvider,
    pub hourly: Vec<ForecastPoint>,
    pub daily_summary: Vec<DailyForecast>,
}

// @spec WX-SRC-OWM-004
impl WeatherForecast {
    /// A forecast of `hourly` points, with its daily summaries on the lawn's local days.
    // @spec WX-SRC-OWM-001, WX-SRC-OWM-002
    pub fn new(
        provider: ForecastProvider,
        fetched_at: DateTime<Utc>,
        hourly: Vec<ForecastPoint>,
        clock: &Clock,
    ) -> Self {
        let daily_summary = daily::summarize(&hourly, clock);
        Self {
            fetched_at,
            provider,
            hourly,
            daily_summary,
        }
    }

    /// Get forecast points for the next N hours
    pub fn next_hours(&self, now: DateTime<Utc>, hours: u32) -> Vec<&ForecastPoint> {
        let cutoff = now + chrono::Duration::hours(hours as i64);
        self.hourly
            .iter()
            .filter(|p| p.timestamp <= cutoff)
            .collect()
    }

    /// Daily summaries that cover enough of the day for their high/low (and the mean
    /// derived from them) to mean something. A forecast starts and ends mid-day, so its
    /// first and last days are usually an evening or a few night hours.
    // @spec WX-SRC-OWM-003
    pub fn full_days(&self) -> impl Iterator<Item = &DailyForecast> {
        self.daily_summary.iter().filter(|d| d.is_full_day())
    }

    /// Daily summaries dated from the start of the feed through `today` + `days` (partial
    /// days included). `today` is the lawn's local date.
    pub fn next_days(&self, today: NaiveDate, days: u32) -> Vec<&DailyForecast> {
        let cutoff = today + chrono::Duration::days(days as i64);
        self.daily_summary
            .iter()
            .filter(|d| d.date <= cutoff)
            .collect()
    }

    /// Whether rain is expected within `hours`: by the amounts the points carry (a point
    /// with no amount adds nothing) or by any point's probability.
    pub fn rain_expected_within(
        &self,
        now: DateTime<Utc>,
        hours: u32,
        threshold_mm: f64,
    ) -> Option<RainForecast> {
        let points = self.next_hours(now, hours);
        let mut total_precip = 0.0;
        let mut max_prob = 0.0;

        for point in points {
            total_precip += point.precipitation_mm.unwrap_or(0.0);
            max_prob = point.precipitation_prob.unwrap_or(0.0).max(max_prob);
        }

        if total_precip >= threshold_mm || max_prob >= 0.5 {
            Some(RainForecast {
                expected_mm: total_precip,
                max_probability: max_prob,
            })
        } else {
            None
        }
    }

    /// Highest daily high over `next_days(today, days)`.
    pub fn max_temp_next_days(&self, today: NaiveDate, days: u32) -> Option<f64> {
        self.next_days(today, days)
            .iter()
            .map(|d| d.high_temp_f)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }
}

#[derive(Debug, Clone)]
pub struct RainForecast {
    pub expected_mm: f64,
    pub max_probability: f64,
}

/// One slot of a forecast. Every provider forecasts temperature, humidity and wind for
/// its whole span; anything else it does not forecast for a slot is `None`, never zero.
// @spec WX-SRC-FC-002, WX-SRC-FC-003
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastPoint {
    /// The start of the slot.
    pub timestamp: DateTime<Utc>,
    /// The slot's length: 1 or 3 hours, by provider.
    pub slot_hours: u32,
    pub temp_f: f64,
    pub feels_like_f: Option<f64>,
    pub humidity_percent: f64,
    /// Rain plus snow over the slot.
    pub precipitation_mm: Option<f64>,
    /// 0.0–1.0.
    pub precipitation_prob: Option<f64>,
    pub wind_speed_mph: f64,
    pub wind_gust_mph: Option<f64>,
    pub cloud_cover_percent: Option<f64>,
    pub weather_condition: WeatherCondition,
}

/// One of the lawn's local days of a forecast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyForecast {
    pub date: NaiveDate,
    pub high_temp_f: f64,
    pub low_temp_f: f64,
    pub avg_humidity: f64,
    /// `None` unless every slot of the day carries an amount.
    pub total_precipitation_mm: Option<f64>,
    /// The highest probability among the slots that carry one.
    pub max_precipitation_prob: Option<f64>,
    pub dominant_condition: WeatherCondition,
    pub avg_wind_speed_mph: f64,
    pub max_wind_gust_mph: Option<f64>,
    /// Hours of this local day the forecast covers: the sum of its slots' lengths.
    #[serde(default)]
    pub hours_covered: u32,
}

/// A day needs this much coverage before its high/low describe the day.
const FULL_DAY_HOURS: u32 = 18;

impl DailyForecast {
    // @spec WX-SRC-OWM-003
    pub fn is_full_day(&self) -> bool {
        self.hours_covered >= FULL_DAY_HOURS
    }
}

/// The conditions every provider's weather is mapped to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum WeatherCondition {
    #[default]
    Clear,
    Clouds,
    Rain,
    Drizzle,
    Thunderstorm,
    Snow,
    Mist,
    Fog,
    Other,
}

impl WeatherCondition {
    pub fn from_owm_id(id: u32) -> Self {
        match id {
            200..=232 => WeatherCondition::Thunderstorm,
            300..=321 => WeatherCondition::Drizzle,
            500..=531 => WeatherCondition::Rain,
            600..=622 => WeatherCondition::Snow,
            701 => WeatherCondition::Mist,
            741 => WeatherCondition::Fog,
            800 => WeatherCondition::Clear,
            801..=804 => WeatherCondition::Clouds,
            _ => WeatherCondition::Other,
        }
    }

    /// From a WMO weather code, as Open-Meteo gives it.
    pub fn from_wmo_code(code: u32) -> Self {
        match code {
            0 | 1 => WeatherCondition::Clear,
            2 | 3 => WeatherCondition::Clouds,
            45 | 48 => WeatherCondition::Fog,
            51..=57 => WeatherCondition::Drizzle,
            61..=67 | 80..=82 => WeatherCondition::Rain,
            71..=77 | 85 | 86 => WeatherCondition::Snow,
            95..=99 => WeatherCondition::Thunderstorm,
            _ => WeatherCondition::Other,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            WeatherCondition::Clear => "Clear",
            WeatherCondition::Clouds => "Cloudy",
            WeatherCondition::Rain => "Rain",
            WeatherCondition::Drizzle => "Drizzle",
            WeatherCondition::Thunderstorm => "Thunderstorm",
            WeatherCondition::Snow => "Snow",
            WeatherCondition::Mist => "Mist",
            WeatherCondition::Fog => "Fog",
            WeatherCondition::Other => "Other",
        }
    }
}

impl std::fmt::Display for WeatherCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod daily_tests;
#[cfg(test)]
mod day_tests;
#[cfg(test)]
mod tests;
