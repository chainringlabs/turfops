//! The fetch schedule: one background task that, once a minute and whenever woken, asks
//! the record what is due and runs those fetches one at a time, the present before the
//! past.

use super::fetcher::{Clients, Fetcher};
use super::order::{ordered, Failing};
use crate::db::weather_queries::{catalog, lawn, plan};
use crate::error::Result;
use crate::logic::soil_estimate::estimator::SoilEstimator;
use crate::logic::weather_record::due::Fetch;
use crate::logic::weather_record::plan::{build, Lawn};
use crate::logic::weather_record::record::WeatherRecord;
use crate::models::weather_record::Network;
use crate::models::LawnLocation;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use chrono_tz::Tz;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// How long the schedule rests between passes when nothing wakes it.
const REST: std::time::Duration = std::time::Duration::from_secs(60);
/// While history is still arriving, the estimate is brought up to date this often.
const ESTIMATE_EVERY: Duration = Duration::hours(1);

/// Starts a pass now instead of at the next minute.
// @spec SYNC-SCHED-008
#[derive(Clone)]
pub struct Waker(Arc<Notify>);

impl Waker {
    pub fn wake(&self) {
        self.0.notify_one();
    }
}

/// Tells the schedule to start no further fetch and end.
// @spec SYNC-SCHED-010
#[derive(Clone)]
pub struct Stopper {
    stopping: Arc<AtomicBool>,
    wake: Arc<Notify>,
}

impl Stopper {
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.wake.notify_one();
    }
}

/// What a pass left to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    /// Nothing more until the next minute or a wake.
    Rest,
    /// More is waiting: run another pass at once, so current data is looked at again
    /// before the next piece of history.
    Again,
}

pub struct Schedule {
    pool: PgPool,
    record: WeatherRecord,
    estimator: SoilEstimator,
    fetcher: Fetcher,
    wake: Arc<Notify>,
    stopping: Arc<AtomicBool>,
    /// How each fetched thing has been failing, by source and product.
    failing: HashMap<String, Failing>,
    registered_on: Option<NaiveDate>,
    /// Whether rows the estimate rests on have arrived since it was last brought up to
    /// date, and when that was.
    estimate_stale: bool,
    estimated_at: Option<DateTime<Utc>>,
    /// The backfill fetch run last, so one that succeeds without holding more is not
    /// run again at once.
    last_backfill: Option<Fetch>,
}

impl Schedule {
    pub fn new(pool: PgPool, clients: Clients) -> (Self, Waker, Stopper) {
        let wake = Arc::new(Notify::new());
        let stopping = Arc::new(AtomicBool::new(false));
        let schedule = Self {
            record: WeatherRecord::new(pool.clone()),
            estimator: SoilEstimator::new(pool.clone()),
            fetcher: Fetcher::new(pool.clone(), clients),
            pool,
            wake: wake.clone(),
            stopping: stopping.clone(),
            failing: HashMap::new(),
            registered_on: None,
            estimate_stale: true,
            estimated_at: None,
            last_backfill: None,
        };
        (schedule, Waker(wake.clone()), Stopper { stopping, wake })
    }

    /// Runs until stopped. A pass that fails is logged and the next one tried.
    // @spec SYNC-SCHED-001, SYNC-SCHED-010
    pub async fn run(mut self) {
        while !self.stopping.load(Ordering::SeqCst) {
            let pass = self.pass(Utc::now()).await.unwrap_or_else(|error| {
                tracing::error!(?error, "weather schedule pass failed");
                Pass::Rest
            });
            if pass == Pass::Rest {
                tokio::select! {
                    _ = self.wake.notified() => {}
                    _ = tokio::time::sleep(REST) => {}
                }
            }
        }
        tracing::info!("Weather schedule stopped");
    }

    /// One pass: bring the plan and the estimate in line, then run what is due.
    // @spec SYNC-SCHED-002, SYNC-SCHED-003, SYNC-SCHED-004, SYNC-SCHED-011
    pub async fn pass(&mut self, now: DateTime<Utc>) -> Result<Pass> {
        let Some((location, zone)) = self.location().await? else {
            // No location: no plan, nothing due, no outside request.
            return Ok(Pass::Rest);
        };
        let today = now.with_timezone(&zone).date_naive();
        let candidate = now.timestamp_subsec_nanos() / 1_000 % 60;
        let minute = lawn::fetch_minute(&self.pool, candidate)
            .await?
            .unwrap_or(0);

        let replanned = self.keep_plan(&location).await?;
        if replanned || self.registered_on != Some(today) {
            self.record.register_wanted(today).await?;
            self.registered_on = Some(today);
        }
        self.keep_estimate(today, now, replanned).await?;

        let due = self.record.due(now, today, minute).await?;
        let mut refreshed = false;
        for network in due.catalogs {
            refreshed |= self.refresh_catalog(network, now).await;
        }
        if refreshed {
            // New stations may change the plan before anything else is fetched.
            return Ok(Pass::Again);
        }
        let (current, backfill) = ordered(due.fetches);
        for fetch in &current {
            self.attempt(fetch, now).await;
        }
        let next = backfill.into_iter().find(|fetch| self.may_try(fetch, now));
        match next {
            Some(fetch) if self.last_backfill.as_ref() != Some(&fetch) => {
                self.attempt(&fetch, now).await;
                self.last_backfill = Some(fetch);
                Ok(Pass::Again)
            }
            Some(_) => Ok(Pass::Rest),
            None => {
                self.last_backfill = None;
                self.record.drop_replaced().await?;
                Ok(Pass::Rest)
            }
        }
    }

    async fn location(&self) -> Result<Option<(LawnLocation, Tz)>> {
        let profile = crate::db::queries::get_default_lawn_profile(&self.pool).await?;
        Ok(profile
            .and_then(|profile| profile.location)
            .and_then(|location| {
                let zone = location.timezone.parse::<Tz>().ok()?;
                Some((location, zone))
            }))
    }

    /// Rebuilds the plan when what it is built from has changed: the location, the
    /// catalog, or the lawn's elevation becoming known. Returns whether it changed.
    // @spec WX-REC-PLAN-002, WX-REC-PLAN-013
    async fn keep_plan(&self, location: &LawnLocation) -> Result<bool> {
        let lawn = Lawn {
            latitude: location.latitude,
            longitude: location.longitude,
            elevation_ft: lawn::elevation_ft(&self.pool).await?,
        };
        let stations = catalog::stations(&self.pool).await?;
        let wanted = build(Some(&lawn), &stations, &[], &[]);
        let held = plan::read(&self.pool).await?;
        let mut wanted_places: Vec<_> = wanted
            .iter()
            .map(|r| (r.input.as_str(), r.rank, r.source.key.as_str(), r.pinned))
            .collect();
        let mut held_places: Vec<_> = held
            .iter()
            .map(|p| (p.input.as_str(), p.rank, p.source.key.as_str(), p.pinned))
            .collect();
        wanted_places.sort_unstable();
        held_places.sort_unstable();
        if wanted_places == held_places {
            return Ok(false);
        }
        plan::replace(&self.pool, &wanted).await?;
        Ok(true)
    }

    /// Brings the soil estimate up to date when rows it rests on have arrived: at once
    /// on a new plan, hourly while history is still arriving.
    async fn keep_estimate(
        &mut self,
        today: NaiveDate,
        now: DateTime<Utc>,
        replanned: bool,
    ) -> Result<()> {
        let waited = self
            .estimated_at
            .is_none_or(|at| now - at >= ESTIMATE_EVERY);
        if replanned || (self.estimate_stale && waited) {
            self.estimator.run(today, now).await?;
            self.estimate_stale = false;
            self.estimated_at = Some(now);
        }
        Ok(())
    }

    fn key(fetch: &Fetch) -> String {
        format!("{}:{}", fetch.source.key, fetch.product.as_str())
    }

    fn may_try(&self, fetch: &Fetch, now: DateTime<Utc>) -> bool {
        self.failing
            .get(&Self::key(fetch))
            .is_none_or(|failing| failing.may_try(now))
    }

    /// Runs one fetch unless the schedule is stopping or the fetch is waiting out a
    /// failure. A failure is kept to its own source and product.
    // @spec SYNC-SCHED-006, SYNC-SCHED-007, SYNC-SCHED-010
    async fn attempt(&mut self, fetch: &Fetch, now: DateTime<Utc>) {
        if self.stopping.load(Ordering::SeqCst) || !self.may_try(fetch, now) {
            return;
        }
        let failing = self.failing.entry(Self::key(fetch)).or_default();
        match self.fetcher.run(fetch, now).await {
            Ok(()) => {
                failing.succeeded();
                self.estimate_stale = true;
            }
            Err(_) => failing.failed(now),
        }
    }

    /// Reads a network's station list again. Returns whether it was replaced.
    async fn refresh_catalog(&mut self, network: Network, now: DateTime<Utc>) -> bool {
        let key = format!("catalog:{}", network.as_str());
        let failing = self.failing.entry(key).or_default();
        if self.stopping.load(Ordering::SeqCst) || !failing.may_try(now) {
            return false;
        }
        match self.fetcher.refresh_catalog(network).await {
            Ok(()) => {
                failing.succeeded();
                true
            }
            Err(error) => {
                tracing::warn!(
                    network = network.as_str(),
                    ?error,
                    "station list refresh failed"
                );
                failing.failed(now);
                false
            }
        }
    }
}
