//! The record as its consumers ask it: one lawn, its plan, and the rows behind each
//! source. This file gathers what resolution needs; the contracts are in `daily.rs`.

use super::readings::{air, scalar, Reading};
use super::resolve::{self, Candidate, DayAir, Resolved};
use crate::clock::Moment;
use crate::db::weather_queries::days;
use crate::db::weather_queries::plan::{self, Placed};
use crate::db::weather_queries::rows;
use crate::error::{Result, TurfOpsError};
use crate::logic::soil_estimate::anchor::anchored;
use crate::models::weather_record::{HourlyRow, Input, Source, SourceKind};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use chrono_tz::Tz;
use sqlx::PgPool;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// How old a source's newest value may be while the source still counts as reporting.
/// Data sync sets these.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgeLimits {
    pub sensor_air: Duration,
    pub sensor_soil: Duration,
    pub analysis: Duration,
    pub station: Duration,
    /// Measured from the end of the day a daily value is for.
    pub daily: Duration,
}

impl AgeLimits {
    /// The limit for a value of `input` from a source of `kind`; `None` is no limit.
    pub fn of(&self, kind: SourceKind, input: Input) -> Option<Duration> {
        match (kind, input) {
            (SourceKind::Sensor, Input::Rain) => None,
            (SourceKind::Sensor, Input::AirTemp | Input::Humidity | Input::DailyAir) => {
                Some(self.sensor_air)
            }
            (SourceKind::Sensor, _) => Some(self.sensor_soil),
            (SourceKind::Analysis, _) => Some(self.analysis),
            (SourceKind::Station, _) => Some(self.station),
            (SourceKind::Grid | SourceKind::Estimate, _) => Some(self.daily),
        }
    }

    fn longest(&self) -> Duration {
        [
            self.sensor_air,
            self.sensor_soil,
            self.analysis,
            self.station,
            self.daily,
        ]
        .into_iter()
        .max()
        .unwrap_or_else(Duration::zero)
    }
}

/// What a caller brings to a question: the lawn's clock reading and the age limits.
// @spec WX-REC-API-001
#[derive(Debug, Clone, Copy)]
pub struct Asked {
    pub at: Moment,
    pub limits: AgeLimits,
}

pub struct WeatherRecord {
    pub(super) pool: PgPool,
}

/// One question's view of the record: the lawn's zone and the plan in force.
pub(super) struct View<'a> {
    pub record: &'a WeatherRecord,
    pub zone: Tz,
    pub plan: Vec<Placed>,
    pub asked: Asked,
    /// Each hourly source's recent hours, read once per question.
    pub(super) recent: tokio::sync::Mutex<HashMap<i64, Arc<Vec<HourlyRow>>>>,
}

/// A resolved series and the sources it was resolved among.
pub(super) struct Resolution<T> {
    pub days: BTreeMap<NaiveDate, Resolved<T>>,
    pub candidates: Vec<Candidate<T>>,
}

impl WeatherRecord {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The view for one question, or the location-not-set outage.
    // @spec WX-REC-FAIL-001
    pub(super) async fn view(&self, asked: Asked) -> Result<View<'_>> {
        let zone = crate::db::queries::get_default_lawn_profile(&self.pool)
            .await?
            .and_then(|profile| profile.location)
            .ok_or_else(TurfOpsError::location_not_set)?
            .timezone
            .parse::<Tz>()
            .map_err(|_| TurfOpsError::Internal("the lawn's timezone is not a zone".into()))?;
        Ok(View {
            record: self,
            zone,
            plan: plan::read(&self.pool).await?,
            asked,
            recent: Default::default(),
        })
    }
}

/// How far either side of a span the measured days that anchor the estimate are looked
/// for. A gap longer than this is anchored from the edge that is in reach.
const ANCHOR_REACH_DAYS: i64 = 366;

/// Replaces each day the soil estimate speaks for with its anchored value: the estimate
/// shifted to meet the measured days around it.
// @spec WX-REC-RES-006
fn anchor(resolution: &mut Resolution<f64>) {
    let kind_of = |rank: usize| resolution.candidates[rank].source.kind;
    let Some(estimate) =
        (0..resolution.candidates.len()).find(|rank| kind_of(*rank) == SourceKind::Estimate)
    else {
        return;
    };
    let measured: BTreeMap<NaiveDate, f64> = resolution
        .days
        .iter()
        .filter(|(_, value)| value.from != estimate)
        .map(|(day, value)| (*day, value.value))
        .collect();
    let anchored = anchored(&resolution.candidates[estimate].covered, &measured);
    for (day, value) in resolution
        .days
        .iter_mut()
        .filter(|(_, v)| v.from == estimate)
    {
        if let Some(shifted) = anchored.get(day) {
            value.value = *shifted;
        }
    }
}

impl View<'_> {
    /// A scalar input resolved for each day from `from` through `through`.
    pub async fn resolve(
        &self,
        input: Input,
        from: NaiveDate,
        through: NaiveDate,
    ) -> Result<Resolution<f64>> {
        let estimated = |kind| self.sources(input).any(|source| source.kind == kind);
        let anchors = matches!(input, Input::SoilTemp5 | Input::SoilTemp10)
            && estimated(SourceKind::Estimate)
            && (estimated(SourceKind::Station) || estimated(SourceKind::Sensor));
        if !anchors {
            return self.resolved(input, scalar(input), from, through).await;
        }
        // The offsets rest on the measured days around a gap, which may lie outside
        // the span asked for.
        let yesterday = self.asked.at.today - Duration::days(1);
        let reach = Duration::days(ANCHOR_REACH_DAYS);
        let wide = (from - reach, (through + reach).min(yesterday).max(through));
        let mut resolution = self.resolved(input, scalar(input), wide.0, wide.1).await?;
        anchor(&mut resolution);
        resolution
            .days
            .retain(|day, _| (from..=through).contains(day));
        Ok(resolution)
    }

    /// Daily air resolved for each day from `from` through `through`.
    pub async fn resolve_air(
        &self,
        from: NaiveDate,
        through: NaiveDate,
    ) -> Result<Resolution<DayAir>> {
        self.resolved(Input::DailyAir, air(), from, through).await
    }

    /// The plan's sources for `input`, in order. Only sources that speak for the lawn
    /// are in the plan.
    // @spec WX-REC-RES-009
    pub fn sources(&self, input: Input) -> impl Iterator<Item = &Source> {
        self.plan
            .iter()
            .filter(move |placed| placed.input == input)
            .map(|placed| &placed.source)
    }

    /// How far back a source's newest value is looked for: past the longest age limit.
    pub fn look_back(&self) -> DateTime<Utc> {
        self.asked.at.now - self.asked.limits.longest() - Duration::days(2)
    }

    async fn resolved<T: Clone>(
        &self,
        input: Input,
        reading: Reading<T>,
        from: NaiveDate,
        through: NaiveDate,
    ) -> Result<Resolution<T>> {
        let mut candidates = Vec::new();
        for source in self.sources(input) {
            candidates.push(
                self.candidate(source, input, &reading, from, through)
                    .await?,
            );
        }
        let rule = resolve::pending_rule(input);
        let days = resolve::resolve(&candidates, from, through, rule);
        Ok(Resolution { days, candidates })
    }

    async fn candidate<T>(
        &self,
        source: &Source,
        input: Input,
        reading: &Reading<T>,
        from: NaiveDate,
        through: NaiveDate,
    ) -> Result<Candidate<T>> {
        let pool = &self.record.pool;
        let now = self.asked.at.now;
        let limit = self.asked.limits.of(source.kind, input);
        let look_back = self.look_back();
        let (covered, newest) = if source.publishes_daily() {
            let shift = Duration::days(reading.shift_days);
            let rows = rows::daily(pool, source.id, from - shift, through - shift).await?;
            let recent =
                rows::daily(pool, source.id, look_back.date_naive(), now.date_naive()).await?;
            // A daily value is as old as the end of the lawn day it is for.
            let newest = recent
                .iter()
                .rev()
                .find(|row| (reading.daily)(row).is_some())
                .map(|row| row.day + shift)
                .map(|day| (day, days::start_of_day(self.zone, day + Duration::days(1))));
            let covered = resolve::covered_by_days(&rows, reading.shift_days, reading.daily);
            (covered, newest)
        } else {
            let hours = days::local_days(pool, source.id, self.zone, from, through).await?;
            let newest = (reading.newest)(&days::newest_hours(pool, source.id, look_back).await?)
                .map(|at| (at.with_timezone(&self.zone).date_naive(), at));
            (resolve::covered_by_hours(&hours, reading.hourly), newest)
        };
        // Only a measured source is waited for. A grid or the analysis that is late is
        // filled from the next estimate at once: that is what the next one is for.
        let measured = matches!(source.kind, SourceKind::Sensor | SourceKind::Station);
        let reached = newest
            .filter(|_| measured)
            .filter(|(_, at)| limit.is_none_or(|limit| now - *at <= limit))
            .map(|(day, _)| day);
        Ok(Candidate {
            source: source.clone(),
            covered,
            reached,
        })
    }
}
