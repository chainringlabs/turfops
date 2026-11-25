//! The summary from what data sync gathered: the record's summary, which inputs the plan
//! has a source for, the newest 10 cm soil value at any age, GDD and the forecast held.
//! No I/O.

use crate::logic::weather_record::live::LIVE_INPUTS;
use crate::logic::weather_record::summary::RecordSummary;
use crate::models::weather_basis::Figure;
use crate::models::weather_record::Input;
use crate::models::{
    BasisStanding, CurrentBasis, DataSource, EnvironmentalReading, EnvironmentalSummary, Trend,
    WeatherForecast, WeekBasis,
};
use chrono::{DateTime, Duration, Utc};
use std::collections::BTreeMap;

/// A forecast older than this is never served.
pub(crate) const FORECAST_MAX_AGE: Duration = Duration::hours(6);

/// What one assembly reads.
#[derive(Debug, Clone)]
pub(crate) struct Gathered {
    pub record: RecordSummary,
    /// The inputs the basis plan has at least one source for.
    pub planned: Vec<Input>,
    /// When the newest 10 cm soil value any source in the plan holds was observed,
    /// whatever its age.
    pub soil_last_seen: Option<DateTime<Utc>>,
    pub gdd: Option<f64>,
    /// The newest forecast the schedule holds, however old.
    pub forecast: Option<WeatherForecast>,
}

/// The summary as of `now`.
// @spec SYNC-MERGE-003, SYNC-MERGE-006, SYNC-MERGE-007, SYNC-MERGE-009, SYNC-MERGE-010
// @spec SYNC-AGE-003, SYNC-FAIL-006, SYNC-FAIL-007
pub(crate) fn assemble(gathered: Gathered, now: DateTime<Utc>) -> EnvironmentalSummary {
    let Gathered {
        record,
        planned,
        soil_last_seen,
        gdd,
        forecast,
    } = gathered;
    let live = &record.live;
    let value = |input| live.get(input).map(|held| held.value);
    // An input the plan has a source for but no current value is withheld; so is every
    // weekly figure built on it.
    let withheld = |input| planned.contains(&input) && live.get(input).is_none();
    let week = |input, figure: Option<Figure<f64>>| figure.filter(|_| !withheld(input));

    let mut current = EnvironmentalReading::new(DataSource::Cached);
    current.timestamp = now;
    current.ambient_temp_f = value(Input::AirTemp);
    current.humidity_percent = value(Input::Humidity);
    current.precipitation_mm = value(Input::Rain);
    current.soil_temp_5_f = value(Input::SoilTemp5);
    current.soil_temp_10_f = value(Input::SoilTemp10);
    current.soil_temp_20_f = value(Input::SoilTemp20);
    current.soil_temp_50_f = value(Input::SoilTemp50);
    current.soil_temp_100_f = value(Input::SoilTemp100);
    current.soil_moisture_5 = value(Input::SoilMoisture5);
    current.soil_moisture_10 = value(Input::SoilMoisture10);
    current.soil_moisture_20 = value(Input::SoilMoisture20);
    current.soil_moisture_50 = value(Input::SoilMoisture50);
    current.soil_moisture_100 = value(Input::SoilMoisture100);

    let current_bases = LIVE_INPUTS
        .iter()
        .map(|&input| match live.get(input) {
            Some(held) => CurrentBasis {
                input: input.as_str().to_string(),
                standing: BasisStanding::InUse,
                basis: Some(held.basis.clone()),
                observed_at: Some(held.observed_at),
            },
            None => CurrentBasis {
                input: input.as_str().to_string(),
                standing: if planned.contains(&input) {
                    BasisStanding::NoCurrentValue
                } else {
                    BasisStanding::NoSource
                },
                basis: None,
                observed_at: None,
            },
        })
        .collect();

    let soil_week = week(Input::SoilTemp10, record.soil_temp_7day_avg_f);
    let air_week = week(Input::AirTemp, record.ambient_temp_7day_avg_f);
    let humidity_week = week(Input::Humidity, record.humidity_7day_avg);
    let rain_week = week(Input::Rain, record.precipitation_7day_total_mm);
    let mut week_bases = BTreeMap::new();
    for (name, figure) in [
        ("soil_temp_7day_avg_f", &soil_week),
        ("ambient_temp_7day_avg_f", &air_week),
        ("humidity_7day_avg", &humidity_week),
        ("precipitation_7day_total_mm", &rain_week),
    ] {
        if let Some(figure) = figure {
            week_bases.insert(
                name.to_string(),
                WeekBasis {
                    basis: figure.basis.clone(),
                    mixed: figure.mixed,
                },
            );
        }
    }

    let soil_in_use = live.get(Input::SoilTemp10);
    let soil_stale =
        soil_in_use.is_none() && withheld(Input::SoilTemp10) && soil_last_seen.is_some();
    let soil_observed_at = match soil_in_use {
        Some(held) => Some(held.observed_at),
        None if withheld(Input::SoilTemp10) => soil_last_seen,
        None => None,
    };

    EnvironmentalSummary {
        current: Some(current),
        soil_temp_7day_avg_f: soil_week.map(|f| f.value),
        ambient_temp_7day_avg_f: air_week.map(|f| f.value),
        humidity_7day_avg: humidity_week.map(|f| f.value),
        precipitation_7day_total_mm: rain_week.map(|f| f.value),
        soil_temp_trend: if withheld(Input::SoilTemp10) {
            Trend::Unknown
        } else {
            record.soil_temp_trend
        },
        last_updated: Some(now),
        soil_observed_at,
        forecast: forecast.filter(|f| now - f.fetched_at < FORECAST_MAX_AGE),
        gdd_base50_ytd: gdd,
        soil_stale,
        current_bases,
        week_bases,
    }
}

#[cfg(test)]
#[path = "assemble_tests.rs"]
mod tests;
