//! The fetch schedule: one background task that, once a minute and whenever woken, asks
//! the record what is due and runs those fetches one at a time, the present before the
//! past.

use super::fetcher::{Clients, Fetcher};
use super::forecast::{ForecastKeeper, ForecastSlot};
use super::order::{ordered, Failing};
use super::planning;
use super::sensor_read::SensorReader;
use crate::db::weather_queries::lawn;
use crate::error::Result;
use crate::logic::soil_estimate::estimator::SoilEstimator;
use crate::logic::weather_record::due::Fetch;
use crate::logic::weather_record::record::WeatherRecord;
use crate::models::weather_record::Network;
use crate::models::LawnLocation;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use chrono_tz::Tz;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// How long the schedule rests between passes when nothing wakes it.
const REST: std::time::Duration = std::time::Duration::from_secs(60);
/// While history is still arriving, the estimate is brought up to date this often.
const ESTIMATE_EVERY: Duration = Duration::hours(1);

/// What the schedule and the requests that wake it share: the refresh asked for, the
/// passes run, and the passes that changed current data.
#[derive(Debug, Default)]
struct Signals {
    refresh: AtomicBool,
    passes: AtomicU64,
    changes: AtomicU64,
    passed: Notify,
}

/// Starts a pass now instead of at the next minute, and asks for a refresh pass.
// @spec SYNC-SCHED-008
#[derive(Clone)]
pub struct Waker {
    wake: Arc<Notify>,
    signals: Arc<Signals>,
}

impl Waker {
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// Asks for a pass that reads the sensors and fetches the forecast whatever their
    /// rhythm and does not wait for the installation's minute, and wakes the schedule.
    // @spec SYNC-SCHED-009
    pub fn refresh(&self) {
        self.signals.refresh.store(true, Ordering::SeqCst);
        self.wake();
    }

    /// How many passes have finished.
    pub fn passes(&self) -> u64 {
        self.signals.passes.load(Ordering::SeqCst)
    }

    /// How many passes changed current data: stored sensor readings, kept a forecast,
    /// completed a current fetch or rebuilt the plan.
    // @spec SYNC-MERGE-008
    pub fn changes(&self) -> u64 {
        self.signals.changes.load(Ordering::SeqCst)
    }

    /// Waits until more than `seen` passes have finished, or `within` has gone by.
    /// Returns whether a pass finished.
    pub async fn pass_after(&self, seen: u64, within: std::time::Duration) -> bool {
        let finished = async {
            loop {
                let passed = self.signals.passed.notified();
                if self.passes() > seen {
                    return;
                }
                passed.await;
            }
        };
        tokio::time::timeout(within, finished).await.is_ok()
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
    /// Whether history was still arriving at the last pass.
    backfilling: bool,
    /// Whether this process has said where things stand.
    announced: bool,
    /// Shared with every `Waker`.
    signals: Arc<Signals>,
    /// Reads the owner's Home Assistant sensors.
    sensors: SensorReader,
    /// Fetches and holds the forecast.
    forecast: ForecastKeeper,
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
            backfilling: false,
            announced: false,
            signals: Arc::new(Signals::default()),
            sensors: SensorReader::new(),
            forecast: ForecastKeeper::new(ForecastSlot::new()),
        };
        let waker = Waker {
            wake: wake.clone(),
            signals: schedule.signals.clone(),
        };
        (schedule, waker, Stopper { stopping, wake })
    }

    /// Where the schedule keeps the forecast it fetches, for the summary and the status.
    pub fn forecast_slot(&self) -> ForecastSlot {
        self.forecast.slot()
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
    // @spec SYNC-SCHED-002, SYNC-SCHED-003, SYNC-SCHED-004, SYNC-SCHED-011, SYNC-SCHED-013, SYNC-SCHED-019
    pub async fn pass(&mut self, now: DateTime<Utc>) -> Result<Pass> {
        let refresh = self.signals.refresh.swap(false, Ordering::SeqCst);
        let mut changed = false;
        let outcome = self.pass_inner(now, refresh, &mut changed).await;
        if changed {
            self.signals.changes.fetch_add(1, Ordering::SeqCst);
        }
        self.signals.passes.fetch_add(1, Ordering::SeqCst);
        self.signals.passed.notify_waiters();
        outcome
    }

    /// One pass's work. `refresh` reads the sensors and fetches the forecast whatever
    /// their rhythm and does not wait for the installation's minute; `changed` is set
    /// when current data changed.
    // @spec SYNC-SCHED-009, SYNC-MERGE-008
    async fn pass_inner(
        &mut self,
        now: DateTime<Utc>,
        refresh: bool,
        changed: &mut bool,
    ) -> Result<Pass> {
        let Some((location, zone)) = self.location().await? else {
            // No location: no plan, nothing due, no outside request.
            return Ok(Pass::Rest);
        };
        let today = now.with_timezone(&zone).date_naive();
        let candidate = now.timestamp_subsec_nanos() / 1_000 % 60;
        let minute = lawn::fetch_minute(&self.pool, candidate)
            .await?
            .unwrap_or(0);

        let replanned = planning::rebuild(&self.pool).await?;
        *changed |= replanned;
        if replanned || self.registered_on != Some(today) {
            self.record.register_wanted(today).await?;
            self.registered_on = Some(today);
        }
        self.keep_estimate(today, now, replanned).await?;
        if refresh {
            self.sensors.force();
            self.forecast.force();
        }
        // The owner's sensors come before every other fetch, then the forecast.
        *changed |= self.sensors.run(&self.pool, today, now).await;
        if !self.stopping.load(Ordering::SeqCst) {
            *changed |= self
                .forecast
                .run(
                    &self.pool,
                    self.fetcher.clients(),
                    &location,
                    zone,
                    now,
                    minute,
                )
                .await;
        }

        // A refresh runs what is due without waiting for the installation's minute.
        let due_minute = if refresh { 0 } else { minute };
        let due = self.record.due(now, today, due_minute).await?;
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
            if let Some(rows) = self.attempt(fetch, now).await {
                *changed = true;
                tracing::debug!(source = %fetch.source.name, rows, "Weather fetched");
            }
        }
        let waiting = backfill.len();
        if !self.announced {
            // Once per start, so a restart says where things stand.
            tracing::info!(
                minute_of_hour = minute,
                current_fetches_due = current.len(),
                history_fetches_waiting = waiting,
                "Weather schedule running"
            );
            self.announced = true;
        }
        self.note_history(waiting);
        let next = backfill.into_iter().find(|fetch| self.may_try(fetch, now));
        match next {
            Some(fetch) if self.last_backfill.as_ref() != Some(&fetch) => {
                if let Some(rows) = self.attempt(&fetch, now).await {
                    // @spec DEPLOY-LOG-004
                    tracing::info!(
                        source = %fetch.source.name,
                        from = %fetch.span.0,
                        through = %fetch.span.1,
                        rows,
                        left = waiting - 1,
                        "Weather history fetched"
                    );
                }
                self.last_backfill = Some(fetch);
                Ok(Pass::Again)
            }
            Some(_) => Ok(Pass::Rest),
            None => {
                self.last_backfill = None;
                let dropped = self.record.drop_replaced().await?;
                if dropped > 0 {
                    tracing::info!(
                        sources = dropped,
                        "Weather sources no longer in the plan removed"
                    );
                }
                Ok(Pass::Rest)
            }
        }
    }

    /// Says when history starts arriving and when it is all in. Each fetch in between
    /// has its own line.
    fn note_history(&mut self, waiting: usize) {
        match (self.backfilling, waiting) {
            (false, 1..) => {
                tracing::info!(fetches = waiting, "Weather history is being downloaded");
                self.backfilling = true;
            }
            (true, 0) => {
                tracing::info!("Weather history is complete");
                self.backfilling = false;
            }
            _ => {}
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
    async fn attempt(&mut self, fetch: &Fetch, now: DateTime<Utc>) -> Option<usize> {
        if self.stopping.load(Ordering::SeqCst) || !self.may_try(fetch, now) {
            return None;
        }
        let failing = self.failing.entry(Self::key(fetch)).or_default();
        match self.fetcher.run(fetch, now).await {
            Ok(rows) => {
                failing.succeeded();
                self.estimate_stale = true;
                Some(rows)
            }
            Err(_) => {
                failing.failed(now);
                None
            }
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
                tracing::info!(network = network.as_str(), "Station list refreshed");
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
