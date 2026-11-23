//! Keeping the record filled: what each source in the plan is wanted for, what is due,
//! how each source is doing, and clearing out sources the plan no longer uses.

use super::due::{self, Fetch};
use super::record::WeatherRecord;
use crate::datasources::cadence::Product;
use crate::db::weather_queries::{catalog, fetch_state, plan, sources};
use crate::error::Result;
use crate::models::weather_record::{FetchState, Input, Network, Source, SourceKind, SourceRole};
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::HashMap;

/// Everything the record wants fetched now.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Due {
    /// Networks whose station list should be read again.
    pub catalogs: Vec<Network>,
    pub fetches: Vec<Fetch>,
}

/// How one source in use is doing.
// @spec WX-REC-FILL-005
#[derive(Debug, Clone, PartialEq)]
pub struct SourceStatus {
    pub source_id: i64,
    pub name: String,
    pub kind: SourceKind,
    /// The inputs the plan ranks this source for.
    pub inputs: Vec<Input>,
    pub last_success: Option<DateTime<Utc>>,
    pub last_failure: Option<DateTime<Utc>>,
    /// First and last day held, when the source is fetched and holds anything.
    pub held: Option<(NaiveDate, NaiveDate)>,
    /// The first day wanted, when the source is fetched.
    pub wanted_from: Option<NaiveDate>,
}

impl WeatherRecord {
    /// The plan's sources, each once, with the inputs it is ranked for.
    async fn in_use(&self) -> Result<Vec<(Source, Vec<Input>)>> {
        let mut in_use: Vec<(Source, Vec<Input>)> = Vec::new();
        for placed in plan::read(&self.pool).await? {
            match in_use
                .iter_mut()
                .find(|(source, _)| source.id == placed.source.id)
            {
                Some((_, inputs)) => inputs.push(placed.input),
                None => in_use.push((placed.source, vec![placed.input])),
            }
        }
        Ok(in_use)
    }

    /// Every source the record keeps filled: the plan's, and the stations and grid
    /// points that calibrate the soil estimate.
    async fn fetched(&self) -> Result<Vec<Source>> {
        let mut fetched: Vec<Source> = self.in_use().await?.into_iter().map(|(s, _)| s).collect();
        let calibrating = sources::all(&self.pool).await?;
        fetched.extend(
            calibrating
                .into_iter()
                .filter(|source| source.role == SourceRole::Calibration),
        );
        Ok(fetched)
    }

    async fn states(&self) -> Result<HashMap<i64, FetchState>> {
        let states = fetch_state::all(&self.pool).await?;
        Ok(states.into_iter().map(|s| (s.source_id, s)).collect())
    }

    /// Registers what each fetched source is wanted for. A source new to
    /// the plan starts holding nothing; one already registered keeps what it holds.
    // @spec WX-REC-STORE-008, WX-REC-FILL-006
    pub async fn register_wanted(&self, today: NaiveDate) -> Result<()> {
        let created: HashMap<i64, NaiveDate> =
            sources::created_on(&self.pool).await?.into_iter().collect();
        for source in self.fetched().await? {
            let located_on = created.get(&source.id).copied().unwrap_or(today);
            if let Some((product, from)) = due::wanted_for(&source, today, located_on) {
                fetch_state::want(&self.pool, source.id, product.as_str(), from).await?;
            }
        }
        Ok(())
    }

    /// What should be fetched now, for an installation whose hourly and daily fetches
    /// run at `minute` past the hour. Only sources in the plan are reported, so a lawn
    /// with no location, whose plan is empty, wants nothing.
    // @spec WX-REC-FILL-002, WX-REC-FILL-006, WX-REC-FILL-008
    pub async fn due(&self, now: DateTime<Utc>, today: NaiveDate, minute: u32) -> Result<Due> {
        if self.in_use().await?.is_empty() {
            return Ok(Due::default());
        }
        let mut states = self.states().await?;
        let with_state: Vec<_> = self
            .fetched()
            .await?
            .into_iter()
            .map(|source| {
                let state = states.remove(&source.id);
                (source, state)
            })
            .collect();
        Ok(Due {
            catalogs: self.catalogs_due(now).await?,
            fetches: due::fetches(&with_state, today, now, minute),
        })
    }

    /// The networks whose station list is missing or a month old. Setup asks this
    /// before the lawn has a location, to have stations to show.
    // @spec WX-REC-FILL-008
    pub async fn catalogs_due(&self, now: DateTime<Utc>) -> Result<Vec<Network>> {
        let refreshed: HashMap<Network, DateTime<Utc>> =
            catalog::refreshed(&self.pool).await?.into_iter().collect();
        let every = Product::Catalog.new_data_every();
        Ok(Network::ALL
            .iter()
            .copied()
            .filter(|network| refreshed.get(network).is_none_or(|at| now - *at >= every))
            .collect())
    }

    /// One entry per source in the plan.
    // @spec WX-REC-FILL-005
    pub async fn status(&self) -> Result<Vec<SourceStatus>> {
        let mut states = self.states().await?;
        Ok(self
            .in_use()
            .await?
            .into_iter()
            .map(|(source, inputs)| {
                let state = states.remove(&source.id);
                SourceStatus {
                    source_id: source.id,
                    name: source.name,
                    kind: source.kind,
                    inputs,
                    last_success: state.as_ref().and_then(|s| s.last_success),
                    last_failure: state.as_ref().and_then(|s| s.last_failure),
                    held: state.as_ref().and_then(|s| s.held),
                    wanted_from: state.as_ref().map(|s| s.wanted_from),
                }
            })
            .collect())
    }

    /// Deletes the sources the plan no longer uses, and their rows, once every fetched
    /// source in the plan holds what it is wanted for. Sensors and calibration sources
    /// are never deleted here. Returns how many sources went.
    // @spec WX-REC-FILL-007
    pub async fn drop_replaced(&self) -> Result<usize> {
        let in_use = self.in_use().await?;
        let states = self.states().await?;
        let filled = in_use.iter().all(|(source, _)| {
            match (due::product_of(source), states.get(&source.id)) {
                (None | Some(Product::NrccDays), _) => true,
                (Some(_), Some(state)) => state
                    .held
                    .is_some_and(|(from, _)| from <= state.wanted_from),
                (Some(_), None) => false,
            }
        });
        if in_use.is_empty() || !filled {
            return Ok(0);
        }
        let mut dropped = 0;
        for source in sources::all(&self.pool).await? {
            let kept = source.kind == SourceKind::Sensor
                || source.role == SourceRole::Calibration
                || in_use.iter().any(|(used, _)| used.id == source.id);
            if !kept {
                sources::delete(&self.pool, source.id).await?;
                dropped += 1;
            }
        }
        Ok(dropped)
    }
}
