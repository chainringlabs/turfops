//! The forecast on the schedule: fetched from the chosen provider for the lawn's
//! location, held in memory with its status, and read by the summary and every place
//! that shows source status.

#[cfg(test)]
#[path = "forecast_tests.rs"]
mod tests;

use super::fetcher::Clients;
use super::order::Failing;
use crate::clock::Clock;
use crate::db::setup_queries::{self, Service};
use crate::models::forecast::{ForecastProvider, WeatherForecast};
use crate::models::LawnLocation;
use chrono::{DateTime, Duration, DurationRound, Utc};
use chrono_tz::Tz;
use serde::Serialize;
use sqlx::PgPool;
use std::sync::{Arc, RwLock};

/// The provider the owner chose and the key it would be asked with.
#[derive(Clone, PartialEq, Eq)]
pub struct Choice {
    pub provider: ForecastProvider,
    /// OpenWeatherMap's stored key; never printed.
    pub key: Option<String>,
}

impl std::fmt::Debug for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Choice")
            .field("provider", &self.provider)
            .field("key", &self.key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// How fetching the forecast is going, as every status surface reports it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForecastStatus {
    /// The chosen provider.
    pub provider: ForecastProvider,
    /// When the forecast held was fetched; empty when none is held.
    pub fetched_at: Option<DateTime<Utc>>,
    /// Why the latest fetch failed or could not be made; empty otherwise.
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct Held {
    /// The provider the schedule last fetched for, or found it could not ask.
    provider: Option<ForecastProvider>,
    forecast: Option<WeatherForecast>,
    last_error: Option<String>,
}

/// The newest forecast and its status, written by the schedule and read by requests.
/// Never held across an await, so a request never waits on a fetch.
#[derive(Debug, Clone, Default)]
pub struct ForecastSlot(Arc<RwLock<Held>>);

impl ForecastSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// The newest forecast held, however old.
    pub fn forecast(&self) -> Option<WeatherForecast> {
        self.read().forecast.clone()
    }

    /// The status as reported while `chosen` is the chosen provider: nothing fetched
    /// and no failure until the schedule has fetched for it.
    // @spec SYNC-CONN-007
    pub fn status(&self, chosen: ForecastProvider) -> ForecastStatus {
        let held = self.read();
        if held.provider != Some(chosen) {
            return ForecastStatus {
                provider: chosen,
                fetched_at: None,
                last_error: None,
            };
        }
        ForecastStatus {
            provider: chosen,
            fetched_at: held.forecast.as_ref().map(|f| f.fetched_at),
            last_error: held.last_error.clone(),
        }
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Held> {
        // A writer never panics while holding the lock, so a poisoned one is still whole.
        self.0
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Held> {
        self.0
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A slot holding `forecast` as fetched from its provider (tests).
    #[cfg(test)]
    pub fn holding(forecast: WeatherForecast) -> Self {
        Self(Arc::new(RwLock::new(Held {
            provider: Some(forecast.provider),
            forecast: Some(forecast),
            last_error: None,
        })))
    }
}

/// The status of the forecast as the pages report it: the chosen provider, read from
/// the database, and what the schedule holds for it.
// @spec SYNC-CONN-007
pub async fn status(pool: &PgPool, slot: &ForecastSlot) -> crate::error::Result<ForecastStatus> {
    let chosen = setup_queries::forecast_provider(pool).await?;
    Ok(slot.status(chosen))
}

/// The provider the owner chose and, for OpenWeatherMap, the key stored for it.
async fn chosen(pool: &PgPool) -> crate::error::Result<Choice> {
    let provider = setup_queries::forecast_provider(pool).await?;
    let key = match provider {
        ForecastProvider::OpenWeatherMap => {
            setup_queries::credential(pool, Service::OpenWeatherMap)
                .await?
                .map(|stored| stored.secret)
        }
        ForecastProvider::OpenMeteo | ForecastProvider::NationalWeatherService => None,
    };
    Ok(Choice { provider, key })
}

fn name(provider: ForecastProvider) -> &'static str {
    match provider {
        ForecastProvider::OpenMeteo => "Open-Meteo",
        ForecastProvider::NationalWeatherService => "The National Weather Service",
        ForecastProvider::OpenWeatherMap => "OpenWeatherMap",
    }
}

/// The first instant after `at` that is `minute` minutes past an hour.
fn next_on_the_minute(at: DateTime<Utc>, minute: u32) -> DateTime<Utc> {
    let hour = at.duration_trunc(Duration::hours(1)).unwrap_or(at);
    let candidate = hour + Duration::minutes(i64::from(minute % 60));
    if candidate > at {
        candidate
    } else {
        candidate + Duration::hours(1)
    }
}

/// What one turn of fetching the forecast came to.
#[derive(Debug)]
pub enum Outcome {
    Fetched(WeatherForecast),
    /// The provider's detail is logged; this is the owner sentence.
    Failed(String),
    /// The chosen provider needs a key and none is stored; nothing was asked.
    NeedsKey,
}

/// How a kept outcome is logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Logged {
    Info,
    Debug,
    Warn,
}

/// The sentence the status gives while the chosen provider needs a key it lacks.
// @spec SYNC-SCHED-023
pub fn needs_key(provider: ForecastProvider) -> String {
    format!(
        "{} needs an API key, so there is no forecast. Add the key where you choose the \
         forecast provider, or choose a provider that needs none.",
        name(provider)
    )
}

/// Decides when the forecast is fetched and keeps what each fetch returns.
pub struct ForecastKeeper {
    slot: ForecastSlot,
    failing: Failing,
    /// The choice the latest turn was for; `None` until the first turn after a start.
    kept_for: Option<Choice>,
    /// When the latest fetch that succeeded was made.
    succeeded_at: Option<DateTime<Utc>>,
    /// Whether the next fetch that succeeds is logged at INFO.
    announce: bool,
    /// A refresh asked for the next turn.
    forced: bool,
}

impl ForecastKeeper {
    pub fn new(slot: ForecastSlot) -> Self {
        Self {
            slot,
            failing: Failing::default(),
            kept_for: None,
            succeeded_at: None,
            announce: true,
            forced: false,
        }
    }

    pub fn slot(&self) -> ForecastSlot {
        self.slot.clone()
    }

    /// The next turn fetches whatever the hour or a failure's retry (a refresh).
    // @spec SYNC-SCHED-009
    pub fn force(&mut self) {
        self.forced = true;
    }

    /// Whether to fetch for `choice` at `now`: at once after a start or a change of
    /// provider or key, after a failure on the retry intervals, and otherwise hourly at
    /// `minute` past the hour.
    // @spec SYNC-SCHED-019, SYNC-SCHED-022
    pub fn due(&self, choice: &Choice, now: DateTime<Utc>, minute: u32) -> bool {
        if self.forced || self.kept_for.as_ref() != Some(choice) {
            return true;
        }
        if self.failing.in_a_row > 0 {
            return self.failing.may_try(now);
        }
        self.succeeded_at
            .is_none_or(|at| now >= next_on_the_minute(at, minute))
    }

    /// Keeps a turn's outcome for `choice`: a forecast replaces the one held; a failure
    /// keeps it and sets the status's sentence; a change of provider first discards the
    /// forecast held. Returns how the turn is logged.
    // @spec SYNC-SCHED-020, SYNC-SCHED-021, SYNC-SCHED-022, SYNC-SCHED-023
    pub fn keep(&mut self, choice: &Choice, outcome: Outcome, now: DateTime<Utc>) -> Logged {
        let mut held = self.slot.write();
        if held.provider != Some(choice.provider) {
            // Another provider's forecast must not stand for the one now chosen.
            *held = Held {
                provider: Some(choice.provider),
                ..Held::default()
            };
            if self.kept_for.is_some() {
                self.announce = true;
            }
            self.failing.succeeded();
            self.succeeded_at = None;
        }
        self.kept_for = Some(choice.clone());
        self.forced = false;
        match outcome {
            Outcome::Fetched(forecast) => {
                held.forecast = Some(forecast);
                held.last_error = None;
                self.failing.succeeded();
                self.succeeded_at = Some(now);
                if std::mem::take(&mut self.announce) {
                    Logged::Info
                } else {
                    Logged::Debug
                }
            }
            Outcome::Failed(said) => {
                held.last_error = Some(said);
                self.failing.failed(now);
                self.announce = true;
                Logged::Warn
            }
            Outcome::NeedsKey => {
                held.last_error = Some(needs_key(choice.provider));
                self.failing.failed(now);
                self.announce = true;
                Logged::Warn
            }
        }
    }

    /// One turn: read the choice, and when due, fetch for the lawn's location and keep
    /// the outcome. A database error ends the turn without changing anything. Returns
    /// whether a forecast was fetched.
    // @spec SYNC-SCHED-019, SYNC-SCHED-023, WX-SRC-FC-004
    pub async fn run(
        &mut self,
        pool: &PgPool,
        clients: &Clients,
        location: &LawnLocation,
        zone: Tz,
        now: DateTime<Utc>,
        minute: u32,
    ) -> bool {
        let choice = match chosen(pool).await {
            Ok(choice) => choice,
            Err(error) => {
                tracing::warn!(?error, "could not read the forecast provider");
                return false;
            }
        };
        if !self.due(&choice, now, minute) {
            return false;
        }
        let (latitude, longitude) = (location.latitude, location.longitude);
        let clock = Clock::Zone(zone);
        let fetched = match (choice.provider, choice.key.as_deref()) {
            (ForecastProvider::OpenWeatherMap, None) => None,
            (ForecastProvider::OpenWeatherMap, Some(key)) => Some(
                clients
                    .owm
                    .forecast(latitude, longitude, key, &clock, now)
                    .await,
            ),
            (ForecastProvider::OpenMeteo, _) => Some(
                clients
                    .open_meteo
                    .forecast(latitude, longitude, &clock, now)
                    .await,
            ),
            (ForecastProvider::NationalWeatherService, _) => {
                Some(clients.nws.forecast(latitude, longitude, &clock, now).await)
            }
        };
        let provider = choice.provider;
        let outcome = match fetched {
            None => {
                tracing::warn!(?provider, "Forecast provider needs a key; none is stored");
                Outcome::NeedsKey
            }
            Some(Err(error)) => {
                tracing::warn!(?provider, ?error, "forecast fetch failed");
                Outcome::Failed(error.owner_message())
            }
            Some(Ok(forecast)) => Outcome::Fetched(forecast),
        };
        let covered = match &outcome {
            Outcome::Fetched(forecast) => Some((
                forecast.daily_summary.len(),
                forecast.daily_summary.first().map(|d| d.date),
                forecast.daily_summary.last().map(|d| d.date),
            )),
            _ => None,
        };
        let logged = self.keep(&choice, outcome, now);
        let Some((days, from, through)) = covered else {
            return false;
        };
        match logged {
            Logged::Info => tracing::info!(?provider, days, ?from, ?through, "Forecast fetched"),
            _ => tracing::debug!(?provider, days, ?from, ?through, "Forecast fetched"),
        }
        true
    }
}
