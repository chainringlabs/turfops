//! The daily inputs of the disease models: each day's air temperature and humidity from
//! one instrument, because dew point and humid hours need both from the same place.

use super::hourly::hour_of;
use super::record::{Asked, View, WeatherRecord};
use super::resolve::{self, Candidate, Pending, COVERING_HOURS};
use crate::db::weather_queries::days;
use crate::db::weather_queries::disease_days::{disease_days, DiseaseDay};
use crate::error::Result;
use crate::models::weather_basis::BasisSpan;
use crate::models::weather_record::{Input, Source, SourceKind};
use crate::models::DailyWeather;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use std::collections::BTreeMap;
use std::ops::RangeInclusive;

/// The day in progress: the place of the source reporting it, and its hours so far.
type InProgress = Option<(usize, Option<DiseaseDay>)>;

/// The observed days the disease models run on.
#[derive(Debug, Clone, PartialEq)]
pub struct DiseaseInputs {
    /// Ascending by date. The last day is usually the day in progress.
    pub days: Vec<DailyWeather>,
    /// The newest hour behind `days`: forecast points up to it are not new.
    pub latest_observed: Option<DateTime<Utc>>,
    /// What each day's air and humidity rest on.
    pub basis: Vec<BasisSpan>,
}

impl WeatherRecord {
    /// One row per local day from `start` that a source speaks for. `dawn` and
    /// `afternoon` are the local hours whose presence the caller wants reported.
    // @spec WX-REC-API-010, WX-REC-RES-005, WX-REC-FAIL-003
    pub async fn fetch_daily_disease_inputs(
        &self,
        start: NaiveDate,
        dawn: RangeInclusive<u32>,
        afternoon: RangeInclusive<u32>,
        asked: Asked,
    ) -> Result<DiseaseInputs> {
        let view = self.view(asked).await?;
        let today = asked.at.today;
        let complete = today - Duration::days(1);
        let (candidates, in_progress) = view.disease_candidates(start, &dawn, &afternoon).await?;
        let mut resolved = resolve::resolve(&candidates, start, complete, Pending::LeavesEmpty);
        if let Some((from, Some(value))) = in_progress {
            resolved.insert(today, resolve::Resolved { value, from });
        }
        let rain = view.rain_by_day(start, today).await?;
        Ok(DiseaseInputs {
            latest_observed: resolved.values().map(|day| day.value.newest_hour).max(),
            basis: resolve::spans(Input::AirTemp, &resolved, &candidates),
            days: resolved
                .into_values()
                .map(|day| DailyWeather {
                    precip_mm: rain.get(&day.value.weather.date).copied().unwrap_or(0.0),
                    ..day.value.weather
                })
                .collect(),
        })
    }
}

impl View<'_> {
    /// Each source's covered days, and the day in progress from the first source still
    /// reporting, with that source's place.
    // @spec WX-REC-RES-005, WX-REC-RES-003
    async fn disease_candidates(
        &self,
        start: NaiveDate,
        dawn: &RangeInclusive<u32>,
        afternoon: &RangeInclusive<u32>,
    ) -> Result<(Vec<Candidate<DiseaseDay>>, InProgress)> {
        let today = self.asked.at.today;
        let complete = today - Duration::days(1);
        let mut candidates = Vec::new();
        // The day in progress goes to the first source still reporting, however few
        // of its hours are in.
        let mut in_progress = None;
        for (rank, source) in self.air_and_humidity_sources().into_iter().enumerate() {
            let span = (start, today);
            let held = disease_days(
                &self.record.pool,
                source.id,
                self.zone,
                span,
                dawn,
                afternoon,
            )
            .await?;
            let reached = held
                .last()
                .filter(|day| self.is_current(source, day.newest_hour))
                .map(|day| day.weather.date);
            if in_progress.is_none() && reached.is_some() {
                let today = held.iter().find(|day| day.weather.date == today).cloned();
                in_progress = Some((rank, today));
            }
            let measured = matches!(source.kind, SourceKind::Sensor | SourceKind::Station);
            candidates.push(Candidate {
                source: source.clone(),
                covered: held
                    .into_iter()
                    .filter(|day| day.weather.date <= complete)
                    .filter(|day| day.weather.hours_covered >= COVERING_HOURS as f64)
                    .map(|day| (day.weather.date, day))
                    .collect::<BTreeMap<_, _>>(),
                reached: reached.filter(|_| measured),
            });
        }
        Ok((candidates, in_progress))
    }

    /// The sources that may give both air temperature and humidity, in plan order:
    /// those ranked for air temperature, then any ranked only for humidity.
    fn air_and_humidity_sources(&self) -> Vec<&Source> {
        let mut sources: Vec<&Source> = self.sources(Input::AirTemp).collect();
        for source in self.sources(Input::Humidity) {
            if !sources.iter().any(|held| held.id == source.id) {
                sources.push(source);
            }
        }
        sources
    }

    /// Whether an hour of air and humidity from `source` is within its age limit.
    fn is_current(&self, source: &Source, hour: DateTime<Utc>) -> bool {
        let limit = self.asked.limits.of(source.kind, Input::AirTemp);
        limit.is_none_or(|limit| self.asked.at.now - hour <= limit)
    }

    /// Each day's rain total from the rain basis, through the hour in progress.
    async fn rain_by_day(
        &self,
        from: NaiveDate,
        through: NaiveDate,
    ) -> Result<BTreeMap<NaiveDate, f64>> {
        let first = days::start_of_day(self.zone, from);
        let hours = self
            .hours(Input::Rain, first, hour_of(self.asked.at.now))
            .await?;
        let mut totals = BTreeMap::new();
        for hour in hours.hours.iter().filter(|hour| hour.day <= through) {
            *totals.entry(hour.day).or_insert(0.0) += hour.value;
        }
        Ok(totals)
    }
}
