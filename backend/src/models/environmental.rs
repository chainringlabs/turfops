use super::forecast::WeatherForecast;
use super::weather_basis::Basis;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataSource {
    SoilData,
    HomeAssistant,
    OpenWeatherMap,
    OpenRouter,
    Cached,
    Manual,
    Calculated,
    History,
    Calendar,
    Agronomic,
    MissouriExtension,
    Rotation,
    SoilTestData,
}

impl DataSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            DataSource::SoilData => "NOAA USCRN",
            DataSource::HomeAssistant => "Patio Sensor",
            DataSource::OpenWeatherMap => "OpenWeatherMap",
            DataSource::OpenRouter => "OpenRouter",
            DataSource::Cached => "Cached",
            DataSource::Manual => "Manual",
            DataSource::Calculated => "Calculated",
            DataSource::History => "History",
            DataSource::Calendar => "Calendar",
            DataSource::Agronomic => "Agronomic",
            DataSource::MissouriExtension => "Missouri Extension",
            DataSource::Rotation => "Rotation",
            DataSource::SoilTestData => "Soil Test",
        }
    }
}

impl std::fmt::Display for DataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// @spec WX-SRC-MODEL-003
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalReading {
    pub timestamp: DateTime<Utc>,
    pub source: DataSource,
    pub soil_temp_5_f: Option<f64>,
    pub soil_temp_10_f: Option<f64>,
    pub soil_temp_20_f: Option<f64>,
    pub soil_temp_50_f: Option<f64>,
    pub soil_temp_100_f: Option<f64>,
    pub soil_moisture_5: Option<f64>,
    pub soil_moisture_10: Option<f64>,
    pub soil_moisture_20: Option<f64>,
    pub soil_moisture_50: Option<f64>,
    pub soil_moisture_100: Option<f64>,
    pub ambient_temp_f: Option<f64>,
    pub humidity_percent: Option<f64>,
    pub precipitation_mm: Option<f64>,
}

impl EnvironmentalReading {
    pub fn new(source: DataSource) -> Self {
        Self {
            timestamp: Utc::now(),
            source,
            soil_temp_5_f: None,
            soil_temp_10_f: None,
            soil_temp_20_f: None,
            soil_temp_50_f: None,
            soil_temp_100_f: None,
            soil_moisture_5: None,
            soil_moisture_10: None,
            soil_moisture_20: None,
            soil_moisture_50: None,
            soil_moisture_100: None,
            ambient_temp_f: None,
            humidity_percent: None,
            precipitation_mm: None,
        }
    }

    // @spec WX-SRC-MODEL-004
    pub fn primary_soil_moisture(&self) -> Option<f64> {
        self.soil_moisture_10
            .or(self.soil_moisture_5)
            .or(self.soil_moisture_20)
    }
}

impl Default for EnvironmentalReading {
    fn default() -> Self {
        Self::new(DataSource::Cached)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvironmentalSummary {
    pub current: Option<EnvironmentalReading>,
    pub soil_temp_7day_avg_f: Option<f64>,
    pub ambient_temp_7day_avg_f: Option<f64>,
    pub humidity_7day_avg: Option<f64>,
    pub precipitation_7day_total_mm: Option<f64>,
    pub soil_temp_trend: Trend,
    pub last_updated: Option<DateTime<Utc>>,
    /// When the station actually measured the soil values in `current`. It can trail by
    /// hours; `last_updated` only says when the app last looked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soil_observed_at: Option<DateTime<Utc>>,
    /// Weather forecast data (5-day/3-hour) from OpenWeatherMap
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forecast: Option<WeatherForecast>,
    /// Year-to-date Growing Degree Days (base 50°F)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gdd_base50_ytd: Option<f64>,
    /// The station's newest soil observation is too old to describe today (SYNC-MERGE-006).
    #[serde(default)]
    pub soil_stale: bool,
    /// What each current value rests on, one entry per input a live reading can fill.
    #[serde(default)]
    pub current_bases: Vec<CurrentBasis>,
    /// What each 7-day figure that has a value rests on, keyed by the figure's field name.
    #[serde(default)]
    pub week_bases: BTreeMap<String, PeriodBasis>,
}

impl EnvironmentalSummary {
    /// The basis of the current value of `input` (its request name, such as
    /// `soil_temp_10`), when one is in use.
    pub fn current_basis(&self, input: &str) -> Option<&Basis> {
        self.current_bases
            .iter()
            .find(|b| b.input == input && b.standing == BasisStanding::InUse)
            .and_then(|b| b.basis.as_ref())
    }

    /// Where a current value of `input` came from, for a data point.
    pub fn measured_now(&self, input: &str) -> crate::models::recommendation::Measured<'_> {
        crate::models::recommendation::Measured::now(self.current_basis(input))
    }

    /// Where the 7-day figure `field` (its field name, such as `soil_temp_7day_avg_f`) came
    /// from, for a data point.
    pub fn measured_week(&self, field: &str) -> crate::models::recommendation::Measured<'_> {
        let week = self.week_bases.get(field);
        crate::models::recommendation::Measured::Record {
            basis: week.map(|w| &w.basis),
            mixed: week.is_some_and(|w| w.mixed),
        }
    }

    /// Where the current soil moisture the rules read (`primary_soil_moisture`) came from.
    pub fn measured_soil_moisture(&self) -> crate::models::recommendation::Measured<'_> {
        let current = self.current.as_ref();
        let input = if current.is_some_and(|c| c.soil_moisture_10.is_some()) {
            "soil_moisture_10"
        } else if current.is_some_and(|c| c.soil_moisture_5.is_some()) {
            "soil_moisture_5"
        } else {
            "soil_moisture_20"
        };
        self.measured_now(input)
    }

    /// Where a forecast value came from, for a data point: the provider of the held forecast.
    pub fn measured_forecast(&self) -> crate::models::recommendation::Measured<'static> {
        crate::models::recommendation::Measured::Forecast(
            self.forecast
                .as_ref()
                .map_or("Forecast", |f| f.provider.label()),
        )
    }
}

/// Where an input of the summary stands.
// @spec WX-VIEW-API-007
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BasisStanding {
    /// A current value from a source in the plan.
    InUse,
    /// The plan has a source; none has a current value.
    NoCurrentValue,
    /// The plan has no source for the input.
    NoSource,
}

/// One input's current basis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentBasis {
    /// The input's request name, such as `soil_temp_10`.
    pub input: String,
    pub standing: BasisStanding,
    pub basis: Option<Basis>,
    /// When the value was observed; for a daily source, the start of its day.
    pub observed_at: Option<DateTime<Utc>>,
}

/// The basis of a figure over a period (a 7-day figure, a year-to-date total, years of history).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeriodBasis {
    pub basis: Basis,
    /// The week drew on more than one basis.
    pub mixed: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trend {
    Rising,
    Falling,
    #[default]
    Stable,
    Unknown,
}

impl Trend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Trend::Rising => "↑ Rising",
            Trend::Falling => "↓ Falling",
            Trend::Stable => "→ Stable",
            Trend::Unknown => "? Unknown",
        }
    }

    pub fn is_rising(&self) -> bool {
        matches!(self, Trend::Rising)
    }
}

impl std::fmt::Display for Trend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub fn celsius_to_fahrenheit(c: f64) -> f64 {
    c * 9.0 / 5.0 + 32.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::weather_basis::BasisKind;
    use crate::models::{ForecastProvider, Measured};

    fn basis(kind: BasisKind, source: &str) -> Basis {
        Basis {
            kind,
            source: source.into(),
            distance_miles: None,
            approximate: false,
        }
    }

    fn in_use(input: &str, basis: Basis) -> CurrentBasis {
        CurrentBasis {
            input: input.into(),
            standing: BasisStanding::InUse,
            basis: Some(basis),
            observed_at: None,
        }
    }

    fn source_of(from: Measured<'_>) -> Option<String> {
        match from {
            Measured::Record { basis, .. } => basis.map(|b| b.source.clone()),
            Measured::Forecast(provider) => Some(provider.to_string()),
        }
    }

    // @spec WX-VIEW-BASIS-001
    #[test]
    fn a_data_point_reads_the_basis_of_the_value_it_shows() {
        let mut env = EnvironmentalSummary {
            current_bases: vec![
                in_use("soil_temp_10", basis(BasisKind::Station, "Avondale 2 N")),
                in_use("soil_moisture_5", basis(BasisKind::Sensor, "Bed probe")),
                CurrentBasis {
                    input: "air_temp".into(),
                    standing: BasisStanding::NoCurrentValue,
                    basis: Some(basis(BasisKind::Sensor, "Patio")),
                    observed_at: None,
                },
            ],
            ..Default::default()
        };
        env.week_bases.insert(
            "soil_temp_7day_avg_f".into(),
            PeriodBasis {
                basis: basis(BasisKind::Estimate, "Soil temperature estimate"),
                mixed: true,
            },
        );
        assert_eq!(
            source_of(env.measured_now("soil_temp_10")).as_deref(),
            Some("Avondale 2 N")
        );
        assert_eq!(source_of(env.measured_now("air_temp")), None, "not in use");
        let Measured::Record { basis: week, mixed } = env.measured_week("soil_temp_7day_avg_f")
        else {
            panic!("a record value");
        };
        assert_eq!(
            (week.map(|b| b.source.as_str()), mixed),
            (Some("Soil temperature estimate"), true)
        );

        // Soil moisture is read 10 cm first, then 5 cm, as `primary_soil_moisture` does.
        env.current = Some(EnvironmentalReading {
            soil_moisture_5: Some(0.3),
            ..Default::default()
        });
        assert_eq!(
            source_of(env.measured_soil_moisture()).as_deref(),
            Some("Bed probe")
        );

        assert_eq!(
            source_of(env.measured_forecast()).as_deref(),
            Some("Forecast")
        );
        env.forecast = Some(WeatherForecast {
            fetched_at: Utc::now(),
            provider: ForecastProvider::NationalWeatherService,
            hourly: vec![],
            daily_summary: vec![],
        });
        assert_eq!(
            source_of(env.measured_forecast()).as_deref(),
            Some("National Weather Service")
        );
    }

    #[test]
    fn celsius_to_fahrenheit_known_values() {
        // Freezing point of water
        assert!((celsius_to_fahrenheit(0.0) - 32.0).abs() < 0.001);
        // Boiling point of water
        assert!((celsius_to_fahrenheit(100.0) - 212.0).abs() < 0.001);
        // Body temperature
        assert!((celsius_to_fahrenheit(37.0) - 98.6).abs() < 0.1);
        // -40 is same in both scales
        assert!((celsius_to_fahrenheit(-40.0) - (-40.0)).abs() < 0.001);
    }

    #[test]
    fn agronomic_temperatures() {
        // Pre-emergent window: 50-60°F = ~10-15.5°C
        assert!((celsius_to_fahrenheit(10.0) - 50.0).abs() < 0.1);
        assert!((celsius_to_fahrenheit(15.5) - 59.9).abs() < 0.1);

        // Grub control window: 60-75°F = ~15.5-24°C
        assert!((celsius_to_fahrenheit(15.5) - 59.9).abs() < 0.1);
        assert!((celsius_to_fahrenheit(24.0) - 75.2).abs() < 0.1);

        // Heat stress threshold: 85°F = ~29.4°C
        assert!((celsius_to_fahrenheit(29.4) - 84.9).abs() < 0.2);
    }

    // @spec WX-SRC-MODEL-004
    #[test]
    fn environmental_reading_primary_soil_moisture() {
        let mut reading = EnvironmentalReading::new(DataSource::SoilData);

        // No moisture set - should return None
        assert!(reading.primary_soil_moisture().is_none());

        // Only 20cm set
        reading.soil_moisture_20 = Some(0.30);
        assert_eq!(reading.primary_soil_moisture(), Some(0.30));

        // 5cm before 20cm
        reading.soil_moisture_5 = Some(0.28);
        assert_eq!(reading.primary_soil_moisture(), Some(0.28));

        // 10cm takes precedence
        reading.soil_moisture_10 = Some(0.25);
        assert_eq!(reading.primary_soil_moisture(), Some(0.25));
    }

    #[test]
    fn data_source_display() {
        assert_eq!(DataSource::SoilData.as_str(), "NOAA USCRN");
        assert_eq!(DataSource::HomeAssistant.as_str(), "Patio Sensor");
        assert_eq!(DataSource::OpenWeatherMap.as_str(), "OpenWeatherMap");
        assert_eq!(DataSource::Cached.as_str(), "Cached");
        assert_eq!(DataSource::Manual.as_str(), "Manual");
        assert_eq!(DataSource::Calculated.as_str(), "Calculated");
        assert_eq!(DataSource::History.as_str(), "History");
        assert_eq!(DataSource::Calendar.as_str(), "Calendar");
        assert_eq!(DataSource::Agronomic.as_str(), "Agronomic");
        assert_eq!(DataSource::MissouriExtension.as_str(), "Missouri Extension");
        assert_eq!(DataSource::Rotation.as_str(), "Rotation");
        assert_eq!(DataSource::SoilTestData.as_str(), "Soil Test");
    }

    #[test]
    fn trend_display() {
        assert!(Trend::Rising.as_str().contains("Rising"));
        assert!(Trend::Falling.as_str().contains("Falling"));
        assert!(Trend::Stable.as_str().contains("Stable"));
        assert!(Trend::Unknown.as_str().contains("Unknown"));
    }
}
