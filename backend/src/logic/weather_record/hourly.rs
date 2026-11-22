//! An input hour by hour: each of the lawn's days from the source that speaks for it.
//! A complete day follows the coverage rule; the day in progress follows the live rule.

use super::readings::of_hour;
use super::record::{Asked, View, WeatherRecord};
use crate::db::weather_queries::{days, rows};
use crate::error::Result;
use crate::models::weather_basis::{Basis, BasisSpan, Series};
use crate::models::weather_record::{Input, Source};
use crate::models::{DataSource, EnvironmentalReading};
use chrono::{DateTime, Duration, NaiveDate, Timelike, Utc};
use std::collections::{BTreeMap, HashMap};

/// One hour of an input.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Hour {
    /// The start of the hour.
    pub at: DateTime<Utc>,
    pub day: NaiveDate,
    /// The value for this hour. A day on a daily source gives every hour its value, and
    /// rain an equal share of its total.
    pub value: f64,
    /// The source's place among the input's sources.
    pub rank: usize,
    /// The day's own value, when the day is on a daily source.
    pub of_day: Option<f64>,
}

/// An input's hours that have a value, ascending, and the sources they came from.
pub(super) struct Hours {
    pub input: Input,
    pub sources: Vec<Source>,
    pub hours: Vec<Hour>,
}

/// Who speaks for a day: an hourly source, or a daily one with its value.
enum Speaker {
    Hourly(usize),
    Daily(usize, f64),
}

/// The start of the hour `at` falls in.
pub(super) fn hour_of(at: DateTime<Utc>) -> DateTime<Utc> {
    at.with_minute(0)
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap_or(at)
}

impl Hours {
    /// The basis of the series: successive days on one source collapsed into a span.
    // @spec WX-REC-BASIS-003
    pub fn spans(&self) -> Vec<BasisSpan> {
        let mut spans: Vec<(usize, BasisSpan)> = Vec::new();
        for hour in &self.hours {
            match spans.last_mut() {
                Some((rank, span)) if *rank == hour.rank => span.to = hour.day,
                _ => spans.push((
                    hour.rank,
                    BasisSpan {
                        input: self.input,
                        from: hour.day,
                        to: hour.day,
                        basis: Basis::from(&self.sources[hour.rank]),
                    },
                )),
            }
        }
        spans.into_iter().map(|(_, span)| span).collect()
    }
}

impl View<'_> {
    /// `input` for each hour from `first` through `last` (hour starts) that has a value.
    pub(super) async fn hours(
        &self,
        input: Input,
        first: DateTime<Utc>,
        last: DateTime<Utc>,
    ) -> Result<Hours> {
        let sources: Vec<Source> = self.sources(input).cloned().collect();
        let day_of = |at: DateTime<Utc>| at.with_timezone(&self.zone).date_naive();
        let speakers = self
            .speakers(input, &sources, day_of(first), day_of(last))
            .await?;

        let held = self.held(input, &sources, &speakers, first, last).await?;

        let mut hours = Vec::new();
        let mut at = first;
        while at <= last {
            let day = day_of(at);
            let hour = match speakers.get(&day) {
                Some(Speaker::Hourly(rank)) => held[rank].get(&at).map(|value| Hour {
                    at,
                    day,
                    value: *value,
                    rank: *rank,
                    of_day: None,
                }),
                Some(Speaker::Daily(rank, of_day)) => Some(Hour {
                    at,
                    day,
                    value: match input {
                        Input::Rain => of_day / self.hours_in(day),
                        _ => *of_day,
                    },
                    rank: *rank,
                    of_day: Some(*of_day),
                }),
                None => None,
            };
            hours.extend(hour);
            at += Duration::hours(1);
        }
        Ok(Hours {
            input,
            sources,
            hours,
        })
    }

    /// The hours of `input` from each hourly source that speaks for a day.
    async fn held(
        &self,
        input: Input,
        sources: &[Source],
        speakers: &BTreeMap<NaiveDate, Speaker>,
        first: DateTime<Utc>,
        last: DateTime<Utc>,
    ) -> Result<HashMap<usize, HashMap<DateTime<Utc>, f64>>> {
        let mut held = HashMap::new();
        for speaker in speakers.values() {
            let Speaker::Hourly(rank) = speaker else {
                continue;
            };
            if !held.contains_key(rank) {
                let pool = &self.record.pool;
                let until = last + Duration::hours(1);
                let rows = rows::hourly(pool, sources[*rank].id, first, until).await?;
                let values = rows
                    .iter()
                    .filter_map(|row| Some((row.observed_at, of_hour(row, input)?)))
                    .collect();
                held.insert(*rank, values);
            }
        }
        Ok(held)
    }

    /// Who speaks for each day from `from` through `through`.
    // @spec WX-REC-RES-001, WX-REC-RES-003
    async fn speakers(
        &self,
        input: Input,
        sources: &[Source],
        from: NaiveDate,
        through: NaiveDate,
    ) -> Result<BTreeMap<NaiveDate, Speaker>> {
        let today = self.asked.at.today;
        let speaker = |rank: usize, value: f64| match sources[rank].publishes_daily() {
            true => Speaker::Daily(rank, value),
            false => Speaker::Hourly(rank),
        };
        let mut speakers = BTreeMap::new();
        let complete = through.min(today - Duration::days(1));
        if from <= complete {
            let resolved = self.resolve(input, from, complete).await?;
            for (day, value) in resolved.days {
                speakers.insert(day, speaker(value.from, value.value));
            }
        }
        if (from..=through).contains(&today) {
            if let Some(speaking) = self.speaking(input).await? {
                speakers.insert(today, speaker(speaking.rank, speaking.newest.value));
            }
        }
        Ok(speakers)
    }

    /// How many hours a local day has: 23 or 25 when the clocks change.
    fn hours_in(&self, day: NaiveDate) -> f64 {
        let start = days::start_of_day(self.zone, day);
        let end = days::start_of_day(self.zone, day + Duration::days(1));
        (end - start).num_hours().max(1) as f64
    }
}

fn set(reading: &mut EnvironmentalReading, input: Input, value: f64) {
    let field = match input {
        Input::AirTemp | Input::DailyAir => &mut reading.ambient_temp_f,
        Input::Humidity => &mut reading.humidity_percent,
        Input::Rain => &mut reading.precipitation_mm,
        Input::SoilTemp5 => &mut reading.soil_temp_5_f,
        Input::SoilTemp10 => &mut reading.soil_temp_10_f,
        Input::SoilTemp20 => &mut reading.soil_temp_20_f,
        Input::SoilTemp50 => &mut reading.soil_temp_50_f,
        Input::SoilTemp100 => &mut reading.soil_temp_100_f,
        Input::SoilMoisture5 => &mut reading.soil_moisture_5,
        Input::SoilMoisture10 => &mut reading.soil_moisture_10,
        Input::SoilMoisture20 => &mut reading.soil_moisture_20,
        Input::SoilMoisture50 => &mut reading.soil_moisture_50,
        Input::SoilMoisture100 => &mut reading.soil_moisture_100,
    };
    *field = Some(value);
}

/// The local hour a day on a daily source is given one reading at.
const NOON: u32 = 12;

impl WeatherRecord {
    /// Hourly readings from `start` through `end`, newest first. Each input is resolved
    /// per local day; a day on a daily source yields one reading of that input, at local
    /// noon.
    // @spec WX-REC-API-002, WX-REC-FAIL-003
    pub async fn fetch_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        asked: Asked,
    ) -> Result<Series<EnvironmentalReading>> {
        let view = self.view(asked).await?;
        let (first, last) = (hour_of(start), hour_of(end));
        let mut readings: BTreeMap<DateTime<Utc>, EnvironmentalReading> = BTreeMap::new();
        let mut basis = Vec::new();
        for input in super::live::LIVE_INPUTS {
            let hours = view.hours(input, first, last).await?;
            basis.extend(hours.spans());
            for hour in &hours.hours {
                let value = match hour.of_day {
                    Some(_) if hour.at.with_timezone(&view.zone).hour() != NOON => continue,
                    Some(of_day) => of_day,
                    None => hour.value,
                };
                let reading = readings
                    .entry(hour.at)
                    .or_insert_with(|| EnvironmentalReading {
                        timestamp: hour.at,
                        ..EnvironmentalReading::new(DataSource::Cached)
                    });
                set(reading, input, value);
            }
        }
        Ok(Series {
            rows: readings.into_values().rev().collect(),
            basis,
        })
    }
}
