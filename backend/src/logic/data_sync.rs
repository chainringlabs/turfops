#[cfg(test)]
mod clock_tests;
#[cfg(all(test, feature = "db-tests"))]
mod db_tests;
#[cfg(test)]
mod memo_slot_tests;
mod probes;
#[cfg(test)]
mod refresh_callers_tests;
#[cfg(test)]
mod service_tests;
#[cfg(test)]
mod source_tests;
mod sync_state;
#[cfg(test)]
mod test_double;
#[cfg(test)]
pub(crate) mod test_env;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

use self::probes::{Probe, ProbeCache};
use self::sync_state::{SensorReads, SyncState};
use crate::clock::Clock;
use crate::config::Config;
use crate::datasources::{HomeAssistantClient, WeatherLakeClient};
use crate::logic::weather_sync::forecast::{self, ForecastSlot, ForecastStatus};
use crate::models::EnvironmentalSummary;
use sqlx::PgPool;
use std::sync::Mutex;

pub struct DataSyncService {
    #[allow(dead_code)]
    pool: PgPool,
    weather_client: Option<WeatherLakeClient>,
    homeassistant_client: Option<HomeAssistantClient>,
    /// The forecast the fetch schedule keeps.
    forecast: ForecastSlot,
    state: SyncState,
    probes: Mutex<ProbeCache>,
    clock: Clock,
}

impl DataSyncService {
    // @spec SYNC-REFRESH-001, SYNC-REFRESH-007
    /// Create a new DataSyncService with lazy external connections.
    /// Clients are configured but not tested — connections are validated
    /// on first data fetch, avoiding slow startup when external services are down.
    // @spec WX-SRC-CFG-002, DEPLOY-START-004
    pub async fn initialize(config: &Config, pool: PgPool) -> Self {
        let homeassistant_client = if !config.homeassistant.token.is_empty() {
            tracing::info!(
                url = %config.homeassistant.url,
                "Home Assistant client configured (connection tested on first fetch)"
            );
            Some(HomeAssistantClient::new(config.homeassistant.clone()))
        } else {
            tracing::warn!(
                "Home Assistant token not configured - ambient data will be unavailable"
            );
            None
        };

        let weather_client = Some(WeatherLakeClient::new(
            &config.datalake,
            config.noaa.station_wbanno,
        ));
        tracing::info!("Weather data lake client configured (parquet read on first fetch)");

        Self {
            pool,
            weather_client,
            homeassistant_client,
            forecast: ForecastSlot::new(),
            state: SyncState::default(),
            probes: Mutex::new(ProbeCache::default()),
            clock: config.clock,
        }
    }

    /// The same service reading the forecast the fetch schedule keeps in `slot`.
    pub fn with_forecast(mut self, slot: ForecastSlot) -> Self {
        self.forecast = slot;
        self
    }

    /// Return the cached summary, refreshing the sensor part first when it is over 5
    /// minutes old. The forecast is the one the schedule holds.
    // @spec SYNC-REFRESH-003, SYNC-REFRESH-004, SYNC-REFRESH-005, SYNC-REFRESH-006, SYNC-REFRESH-009
    pub async fn get_or_refresh(&mut self) -> crate::error::Result<EnvironmentalSummary> {
        if self.state.due(self.clock.now()) {
            self.refresh().await;
        }
        Ok(self.summary())
    }

    /// Always read the sensors again, ignoring their age.
    /// Used by the explicit refresh button in the frontend.
    pub async fn force_refresh(&mut self) -> crate::error::Result<EnvironmentalSummary> {
        self.refresh().await;
        Ok(self.summary())
    }

    // @spec SYNC-FAIL-006, SYNC-FAIL-007, SYNC-SCHED-020
    fn summary(&self) -> EnvironmentalSummary {
        self.state
            .summary(self.clock.now(), self.forecast.forecast())
    }

    /// The forecast's status: the chosen provider and what the schedule holds for it.
    // @spec SYNC-CONN-007
    pub async fn forecast_status(&self) -> crate::error::Result<ForecastStatus> {
        forecast::status(&self.pool, &self.forecast).await
    }

    /// The connection status with the forecast's status beside it.
    // @spec SYNC-CONN-008
    pub async fn connections(&self) -> crate::error::Result<Connections> {
        Ok(Connections {
            sources: self.check_connections().await,
            forecast: self.forecast_status().await?,
        })
    }

    /// Connection status, probing only the configured sources whose last result is over
    /// 5 minutes old; the summary and its timers are untouched.
    // @spec SYNC-CONN-001, SYNC-CONN-002
    pub async fn check_connections(&self) -> ConnectionStatus {
        let recent = |probe| self.probes.lock().ok()?.recent(probe, self.clock.now());
        let mut status = ConnectionStatus::default();

        if let Some(ref client) = self.weather_client {
            status.soildata = match recent(Probe::Lake) {
                Some(up) => up,
                None => self.record(Probe::Lake, client.test_connection().await.unwrap_or(false)),
            };
        }
        if let Some(ref client) = self.homeassistant_client {
            status.homeassistant = match recent(Probe::HomeAssistant) {
                Some(up) => up,
                None => self.record(
                    Probe::HomeAssistant,
                    client.test_connection().await.unwrap_or(false),
                ),
            };
        }
        status
    }

    fn record(&self, probe: Probe, up: bool) -> bool {
        if let Ok(mut probes) = self.probes.lock() {
            probes.record(probe, up, self.clock.now());
        }
        up
    }

    /// Provide access to the weather lake client for direct queries (GDD, historical).
    pub fn weather_client(&self) -> Option<&WeatherLakeClient> {
        self.weather_client.as_ref()
    }

    /// The same service with no weather lake, as if none were configured.
    #[cfg(all(test, feature = "db-tests"))]
    pub(crate) fn without_weather_lake(mut self) -> Self {
        self.weather_client = None;
        self
    }

    /// Read the sensors' sources, then store them at once: until every read is in hand
    /// nothing changes, so a request dropped mid-read leaves the part stale.
    // @spec SYNC-REFRESH-008, SYNC-FAIL-001
    async fn refresh(&mut self) {
        let reads = self.read_sensors().await;
        self.state.store_sensors(self.clock.now(), reads);
    }

    async fn read_sensors(&self) -> SensorReads {
        let lake = match self.weather_client {
            Some(ref client) => Some(client.fetch_summary(self.clock.now()).await),
            None => None,
        };
        let home_assistant = match self.homeassistant_client {
            Some(ref client) => Some(client.fetch_current().await),
            None => None,
        };
        let gdd = match self.weather_client {
            Some(ref client) => Some(client.fetch_gdd_ytd(self.clock.today()).await),
            None => None,
        };
        SensorReads {
            lake,
            home_assistant,
            gdd,
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ConnectionStatus {
    pub soildata: bool,
    pub homeassistant: bool,
}

/// The connection status and the forecast's status, as the dashboard and the MCP
/// snapshot report them.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Connections {
    #[serde(flatten)]
    pub sources: ConnectionStatus,
    pub forecast: ForecastStatus,
}
