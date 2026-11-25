mod assemble;
#[cfg(test)]
mod clock_tests;
#[cfg(all(test, feature = "db-tests"))]
mod db_tests;
mod memo;
#[cfg(test)]
mod memo_slot_tests;
#[cfg(all(test, feature = "db-tests"))]
pub(crate) mod record_db_tests;
#[cfg(test)]
mod refresh_callers_tests;
#[cfg(all(test, feature = "db-tests"))]
mod test_double;
#[cfg(test)]
pub(crate) mod test_env;
#[cfg(test)]
pub(crate) mod test_support;

use self::assemble::{assemble, Gathered};
use self::memo::Kept;
use crate::clock::{Clock, Moment};
use crate::config::Config;
use crate::datasources::WeatherLakeClient;
use crate::db::weather_queries::plan;
use crate::logic::weather_record::record::{AgeLimits, Asked, WeatherRecord};
use crate::logic::weather_sync::forecast::{self, ForecastSlot, ForecastStatus};
use crate::logic::weather_sync::schedule::Waker;
use crate::logic::weather_sync::AGE_LIMITS;
use crate::models::weather_record::Input;
use crate::models::EnvironmentalSummary;
use chrono::{DateTime, Duration, Utc};
use chrono_tz::Tz;
use sqlx::PgPool;

/// A refresh asked within this long of the last asks for nothing.
const REFRESH_FLOOR: Duration = Duration::minutes(1);
/// How long a refresh waits for the schedule's pass.
const REFRESH_WAIT: std::time::Duration = std::time::Duration::from_secs(20);
/// Every value counts, however old: for when 10 cm soil was last seen.
const ANY_AGE: AgeLimits = AgeLimits {
    sensor_air: Duration::days(36_500),
    sensor_soil: Duration::days(36_500),
    analysis: Duration::days(36_500),
    station: Duration::days(36_500),
    daily: Duration::days(36_500),
};

/// The summary of present conditions, assembled from the weather record and the forecast
/// the fetch schedule holds, and the status of the sources behind it. It makes no
/// outside request of its own.
pub struct DataSyncService {
    pool: PgPool,
    /// The weather lake. No page reads it; it goes with the lake reader.
    #[allow(dead_code)]
    weather_client: Option<WeatherLakeClient>,
    /// The forecast the fetch schedule keeps.
    forecast: ForecastSlot,
    /// Asks the fetch schedule for a refresh pass; absent where none runs.
    waker: Option<Waker>,
    kept: Kept,
    refreshed_at: Option<DateTime<Utc>>,
    clock: Clock,
    /// Tests of pages that still read the lake: answer with this summary (or an empty
    /// one) and the forecast held, without the record.
    #[cfg(test)]
    lake_summary: bool,
    #[cfg(test)]
    pinned: Option<EnvironmentalSummary>,
}

impl DataSyncService {
    /// Builds the service; nothing outside is contacted.
    // @spec SYNC-REFRESH-007, DEPLOY-START-004
    pub async fn initialize(config: &Config, pool: PgPool) -> Self {
        let weather_client = Some(WeatherLakeClient::new(
            &config.datalake,
            config.noaa.station_wbanno,
        ));
        tracing::info!("Weather data lake client configured (parquet read on first fetch)");
        Self {
            pool,
            weather_client,
            forecast: ForecastSlot::new(),
            waker: None,
            kept: Kept::default(),
            refreshed_at: None,
            clock: config.clock,
            #[cfg(test)]
            lake_summary: false,
            #[cfg(test)]
            pinned: None,
        }
    }

    /// The same service able to ask the fetch schedule for a refresh pass.
    pub fn with_waker(mut self, waker: Waker) -> Self {
        self.waker = Some(waker);
        self
    }

    /// The same service reading the forecast the fetch schedule keeps in `slot`.
    pub fn with_forecast(mut self, slot: ForecastSlot) -> Self {
        self.forecast = slot;
        self
    }

    /// The summary: the memo while it is under a minute old and no pass has changed
    /// current data since, else assembled afresh from the record.
    // @spec SYNC-MERGE-007, SYNC-MERGE-008, SYNC-MERGE-011, SYNC-FAIL-001, SYNC-FAIL-010
    // @spec SYNC-FAIL-011, SYNC-REFRESH-009
    pub async fn get_or_refresh(&mut self) -> crate::error::Result<EnvironmentalSummary> {
        #[cfg(test)]
        if self.lake_summary {
            return self.lake_for_tests().await;
        }
        let now = self.clock.now();
        let changes = self.waker.as_ref().map_or(0, Waker::changes);
        if let Some(summary) = self.kept.reuse(now, changes) {
            return Ok(summary);
        }
        match self.gather(now).await {
            Ok(gathered) => {
                let summary = match gathered {
                    Some(gathered) => assemble(gathered, now),
                    None => EnvironmentalSummary {
                        last_updated: Some(now),
                        ..Default::default()
                    },
                };
                self.kept.assembled(summary.clone(), now, changes);
                Ok(summary)
            }
            Err(error) => {
                tracing::warn!(?error, "could not read the weather record for the summary");
                Ok(self.kept.failed(now))
            }
        }
    }

    /// Asks the schedule for a refresh pass, unless one was asked less than a minute
    /// ago, and returns what to wait on: the waker and the passes finished before it.
    // @spec SYNC-SCHED-009
    pub fn ask_refresh(&mut self) -> Option<(Waker, u64)> {
        let now = self.clock.now();
        self.kept.discard();
        let waker = self.waker.clone()?;
        if self.refreshed_at.is_some_and(|at| now - at < REFRESH_FLOOR) {
            return None;
        }
        self.refreshed_at = Some(now);
        let seen = waker.passes();
        waker.refresh();
        Some((waker, seen))
    }

    /// Asks for a refresh pass, waits up to 20 seconds for it, and returns a summary
    /// assembled after it. A request handler holding a lock on the service should use
    /// [`Self::ask_refresh`] and wait without the lock.
    // @spec SYNC-SCHED-009
    #[cfg(all(test, feature = "db-tests"))]
    pub async fn force_refresh(&mut self) -> crate::error::Result<EnvironmentalSummary> {
        if let Some((waker, seen)) = self.ask_refresh() {
            waker.pass_after(seen, REFRESH_WAIT).await;
            self.kept.discard();
        }
        self.get_or_refresh().await
    }

    /// How long a refresh waits for the schedule's pass.
    pub fn refresh_wait() -> std::time::Duration {
        REFRESH_WAIT
    }

    /// Forget the memo, so the next request assembles afresh.
    pub fn discard_summary(&mut self) {
        self.kept.discard();
    }

    /// Everything one assembly reads, or `None` while the lawn has no location. GDD runs
    /// from Jan 1 of the lawn's local year through its local today.
    // @spec RULE-GDD-002, SYNC-AGE-001, SYNC-AGE-002, SYNC-AGE-004
    async fn gather(&self, now: DateTime<Utc>) -> crate::error::Result<Option<Gathered>> {
        let profile = crate::db::queries::get_default_lawn_profile(&self.pool).await?;
        let Some(zone) = profile
            .and_then(|p| p.location)
            .and_then(|l| l.timezone.parse::<Tz>().ok())
        else {
            return Ok(None);
        };
        let at = Moment {
            now,
            today: now.with_timezone(&zone).date_naive(),
        };
        let record = WeatherRecord::new(self.pool.clone());
        let asked = Asked {
            at,
            limits: AGE_LIMITS,
        };
        let summary = record.fetch_summary(asked).await?;
        let soil_last_seen = record
            .fetch_latest(Asked {
                at,
                limits: ANY_AGE,
            })
            .await?
            .and_then(|reading| reading.get(Input::SoilTemp10).map(|v| v.observed_at));
        let gdd = record
            .fetch_gdd_ytd(Asked {
                at,
                limits: AGE_LIMITS,
            })
            .await?
            .map(|figure| figure.value);
        let mut planned: Vec<Input> = plan::read(&self.pool)
            .await?
            .into_iter()
            .map(|placed| placed.input)
            .collect();
        planned.dedup();
        Ok(Some(Gathered {
            record: summary,
            planned,
            soil_last_seen,
            gdd,
            forecast: self.forecast.forecast(),
        }))
    }

    /// One entry per source in the plan, from the record's status (WX-VIEW-STATUS-001).
    // @spec SYNC-CONN-001, SYNC-CONN-002
    pub async fn sources(&self) -> crate::error::Result<Vec<SourceStatusView>> {
        let statuses = WeatherRecord::new(self.pool.clone()).status().await?;
        Ok(statuses
            .into_iter()
            .map(|status| {
                let fetched = status.last_success.is_some() || status.last_failure.is_some();
                let failing = status
                    .last_failure
                    .is_some_and(|failed| status.last_success.is_none_or(|ok| failed > ok));
                SourceStatusView {
                    id: status.source_id,
                    name: status.name,
                    kind: status.kind,
                    ok: fetched.then_some(!failing),
                    last_success: status.last_success,
                    last_failure: status.last_failure,
                    last_error: status.last_error,
                    inputs: status.inputs.iter().map(Input::as_str).collect(),
                    held_from: status.held.map(|held| held.0),
                    held_through: status.held.map(|held| held.1),
                    wanted_from: status.wanted_from,
                }
            })
            .collect())
    }

    /// The forecast's status: the chosen provider and what the schedule holds for it.
    // @spec SYNC-CONN-007
    pub async fn forecast_status(&self) -> crate::error::Result<ForecastStatus> {
        forecast::status(&self.pool, &self.forecast).await
    }

    /// The weather lake client, when one is configured.
    #[allow(dead_code)]
    pub fn weather_client(&self) -> Option<&WeatherLakeClient> {
        self.weather_client.as_ref()
    }

    /// An empty reading and the forecast held (tests of pages that still read the lake).
    #[cfg(test)]
    async fn lake_for_tests(&self) -> crate::error::Result<EnvironmentalSummary> {
        let now = self.clock.now();
        let Some(client) = &self.weather_client else {
            return Ok(EnvironmentalSummary {
                last_updated: Some(now),
                forecast: self.forecast.forecast(),
                ..Default::default()
            });
        };
        let _ = client;
        let mut summary = self.pinned.clone().unwrap_or_else(|| EnvironmentalSummary {
            // A fixed stamp, so two calls compare equal.
            current: Some(crate::models::EnvironmentalReading {
                timestamp: DateTime::UNIX_EPOCH,
                ..Default::default()
            }),
            ..Default::default()
        });
        summary.forecast = self
            .forecast
            .forecast()
            .filter(|f| now - f.fetched_at < assemble::FORECAST_MAX_AGE);
        summary.last_updated = Some(now);
        Ok(summary)
    }
}

/// What to ask the weather record now, on the lawn's local calendar when it has a
/// location (the record answers location-not-set when it has none).
pub(crate) async fn record_asked(pool: &PgPool, clock: &Clock) -> crate::error::Result<Asked> {
    let now = clock.now();
    let zone = crate::db::queries::get_default_lawn_profile(pool)
        .await?
        .and_then(|p| p.location)
        .and_then(|l| l.timezone.parse::<Tz>().ok());
    let today = zone.map_or_else(
        || clock.today(),
        |zone| now.with_timezone(&zone).date_naive(),
    );
    Ok(Asked {
        at: Moment { now, today },
        limits: AGE_LIMITS,
    })
}

/// The basis plan as one string, so a memo keyed on it is dropped when the plan changes.
pub(crate) async fn plan_key(pool: &PgPool) -> crate::error::Result<String> {
    let placed = crate::db::weather_queries::plan::read(pool).await?;
    Ok(placed
        .iter()
        .map(|p| format!("{}:{}:{}", p.input.as_str(), p.rank, p.source.id))
        .collect::<Vec<_>>()
        .join(","))
}

/// One source in use and how its fetching is going.
// @spec WX-VIEW-STATUS-001
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SourceStatusView {
    pub id: i64,
    pub name: String,
    pub kind: crate::models::weather_record::SourceKind,
    /// Whether its most recent scheduled fetch succeeded; empty until it is fetched.
    pub ok: Option<bool>,
    pub last_success: Option<chrono::DateTime<chrono::Utc>>,
    pub last_failure: Option<chrono::DateTime<chrono::Utc>>,
    /// The owner sentence for a failure newer than the last success.
    pub last_error: Option<String>,
    /// The inputs the plan ranks it for, as named in requests.
    pub inputs: Vec<&'static str>,
    pub held_from: Option<chrono::NaiveDate>,
    pub held_through: Option<chrono::NaiveDate>,
    pub wanted_from: Option<chrono::NaiveDate>,
}
