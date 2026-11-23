//! The live reading: for each input, the newest value from the highest-ranked source
//! that is still reporting it.

use super::readings::{of_hour, scalar};
use super::record::{Asked, View, WeatherRecord};
use crate::db::weather_queries::{days, rows, sensors};
use crate::error::Result;
use crate::models::weather_basis::Basis;
use crate::models::weather_record::{HourlyRow, Input, Source, SourceKind};
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;

/// The inputs a live reading can fill.
pub const LIVE_INPUTS: [Input; 13] = [
    Input::AirTemp,
    Input::Humidity,
    Input::Rain,
    Input::SoilTemp5,
    Input::SoilTemp10,
    Input::SoilTemp20,
    Input::SoilTemp50,
    Input::SoilTemp100,
    Input::SoilMoisture5,
    Input::SoilMoisture10,
    Input::SoilMoisture20,
    Input::SoilMoisture50,
    Input::SoilMoisture100,
];

/// One input's current value.
// @spec WX-REC-RES-004, WX-REC-BASIS-005
#[derive(Debug, Clone, PartialEq)]
pub struct LiveValue {
    pub value: f64,
    /// When it was observed; for a daily source, the start of the day it is for.
    pub observed_at: DateTime<Utc>,
    pub basis: Basis,
}

/// The current value of each input that has one.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveReading {
    pub values: Vec<(Input, LiveValue)>,
}

impl LiveReading {
    pub fn get(&self, input: Input) -> Option<&LiveValue> {
        self.values
            .iter()
            .find(|(held, _)| *held == input)
            .map(|(_, value)| value)
    }

    /// Soil moisture at 10 cm, else 5 cm, else 20 cm.
    // @spec WX-REC-API-012
    pub fn primary_soil_moisture(&self) -> Option<&LiveValue> {
        [
            Input::SoilMoisture10,
            Input::SoilMoisture5,
            Input::SoilMoisture20,
        ]
        .into_iter()
        .find_map(|input| self.get(input))
    }
}

/// A source's newest value of an input.
pub(super) struct Newest {
    pub value: f64,
    pub observed_at: DateTime<Utc>,
    /// The instant its age is measured from: for a daily value, the end of its day.
    aged_from: DateTime<Utc>,
}

/// The source that speaks for an input now, and what it says.
pub(super) struct Speaking {
    /// The source's place among the input's sources.
    pub rank: usize,
    pub newest: Newest,
}

impl WeatherRecord {
    /// The live reading, or nothing when no input has a current value.
    // @spec WX-REC-RES-003, WX-REC-RES-004, WX-REC-FAIL-003
    pub async fn fetch_latest(&self, asked: Asked) -> Result<Option<LiveReading>> {
        let reading = self.view(asked).await?.live_reading().await?;
        Ok((!reading.values.is_empty()).then_some(reading))
    }
}

impl View<'_> {
    pub(super) async fn live_reading(&self) -> Result<LiveReading> {
        let mut values = Vec::new();
        for input in LIVE_INPUTS {
            if let Some(speaking) = self.speaking(input).await? {
                let source = self.sources(input).nth(speaking.rank);
                let basis = source.map(Basis::from).expect("the rank is in the plan");
                values.push((
                    input,
                    LiveValue {
                        value: speaking.newest.value,
                        observed_at: speaking.newest.observed_at,
                        basis,
                    },
                ));
            }
        }
        Ok(LiveReading { values })
    }

    /// The highest-ranked source whose newest value of `input` is within its age limit.
    // @spec WX-REC-RES-003
    pub(super) async fn speaking(&self, input: Input) -> Result<Option<Speaking>> {
        let now = self.asked.at.now;
        for (rank, source) in self.sources(input).enumerate() {
            // A day's rain total says nothing about now: an earlier day's amount is not
            // falling today. Only an hourly source speaks for current rain.
            if input == Input::Rain && source.publishes_daily() {
                continue;
            }
            let Some(newest) = self.newest(source, input).await? else {
                continue;
            };
            let limit = self.asked.limits.of(source.kind, input);
            if limit.is_none_or(|limit| now - newest.aged_from <= limit) {
                let newest = self.met(source, input, newest).await?;
                return Ok(Some(Speaking { rank, newest }));
            }
        }
        Ok(None)
    }

    /// The soil estimate's newest value as it stands after measured days: anchored, so
    /// the reading does not step when a station goes quiet.
    // @spec WX-REC-RES-006
    async fn met(&self, source: &Source, input: Input, newest: Newest) -> Result<Newest> {
        if source.kind != SourceKind::Estimate {
            return Ok(newest);
        }
        let day = newest.observed_at.with_timezone(&self.zone).date_naive();
        let resolved = self.resolve(input, day, day).await?;
        let from_estimate = |rank: usize| resolved.candidates[rank].source.id == source.id;
        let value = resolved
            .days
            .get(&day)
            .filter(|value| from_estimate(value.from))
            .map_or(newest.value, |value| value.value);
        Ok(Newest { value, ..newest })
    }

    /// A source's newest non-null value of `input`. A probe that skipped its last hour
    /// still has the one before.
    async fn newest(&self, source: &Source, input: Input) -> Result<Option<Newest>> {
        let pool = &self.record.pool;
        if source.kind == SourceKind::Sensor {
            // The reading as it arrived, not the hour it was folded into.
            let raw = sensors::latest_reading(pool, source.id, input).await?;
            return Ok(raw.map(|(observed_at, value)| Newest {
                value,
                observed_at,
                aged_from: observed_at,
            }));
        }
        if source.publishes_daily() {
            let reading = scalar(input);
            let shift = Duration::days(reading.shift_days);
            let today = self.asked.at.today;
            let rows = rows::daily(pool, source.id, self.look_back().date_naive(), today).await?;
            return Ok(rows.iter().rev().find_map(|row| {
                let day = row.day + shift;
                Some(Newest {
                    value: (reading.daily)(row)?,
                    observed_at: days::start_of_day(self.zone, day),
                    aged_from: days::start_of_day(self.zone, day + Duration::days(1)),
                })
            }));
        }
        let hours = self.recent_hours(source.id).await?;
        Ok(hours.iter().rev().find_map(|row| {
            Some(Newest {
                value: of_hour(row, input)?,
                observed_at: row.observed_at,
                aged_from: row.observed_at,
            })
        }))
    }

    /// An hourly source's hours since the look-back, ascending.
    pub(super) async fn recent_hours(&self, source_id: i64) -> Result<Arc<Vec<HourlyRow>>> {
        if let Some(held) = self.recent.lock().await.get(&source_id) {
            return Ok(held.clone());
        }
        let until = self.asked.at.now + Duration::hours(1);
        let pool = &self.record.pool;
        let hours = Arc::new(rows::hourly(pool, source_id, self.look_back(), until).await?);
        self.recent.lock().await.insert(source_id, hours.clone());
        Ok(hours)
    }
}
