use crate::clock::Clock;
use crate::datasources::weather::ClimateRecord;
use crate::datasources::OpenRouterClient;
use crate::logic::data_sync::DataSyncService;
use crate::logic::rules::RulesEngine;
use crate::logic::weather_sync::fetcher::Fetcher;
use crate::logic::weather_sync::posted::PostedLog;
use crate::logic::weather_sync::schedule::Waker;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Multi-year daily climate record from the lake with the time its read completed. Unlike
/// the other lake reads this one spans 15 years of hourly data, so it is memoized briefly.
pub type ClimateCache = Arc<RwLock<Option<(chrono::DateTime<chrono::Utc>, Arc<ClimateRecord>)>>>;

/// The seasonal plan's 10 cm crossings from the weather record, kept for an hour unless
/// the basis plan or the lawn's date changes (`key`; `None` holds a test's crossings).
pub type CrossingMemo = Arc<
    RwLock<
        Option<(
            Option<String>,
            chrono::DateTime<chrono::Utc>,
            Arc<crate::logic::seasonal_plan::RecordCrossings>,
        )>,
    >,
>;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub rules_engine: Arc<RulesEngine>,
    // @spec SYNC-REFRESH-006
    pub sync_service: Arc<RwLock<DataSyncService>>,
    pub openrouter: Option<Arc<OpenRouterClient>>,
    pub climate_cache: ClimateCache,
    pub crossing_memo: CrossingMemo,
    /// Whether `/mcp` is mounted (reported by the health check).
    pub mcp_enabled: bool,
    /// The lawn's clock: every "today" and date gate.
    pub clock: Clock,
    /// Starts a pass of the weather fetch schedule; absent where none runs.
    pub weather_waker: Option<Waker>,
    /// Reaches the outside weather sources for what setup asks at once: a place search,
    /// a station list that is not loaded yet.
    pub weather: Option<Arc<Fetcher>>,
    /// Present while `/api/v1/sensor-readings` is mounted: held for the length of each
    /// request, so they are taken one at a time, and what has been logged about them.
    pub sensor_readings: Option<Arc<tokio::sync::Mutex<PostedLog>>>,
}

impl AppState {
    /// The same state, able to reach the outside weather sources.
    pub fn fetching(mut self, fetcher: Fetcher) -> Self {
        self.weather = Some(Arc::new(fetcher));
        self
    }

    /// The same state, taking posted sensor readings.
    // @spec WX-SRC-POST-014
    pub fn posting(mut self) -> Self {
        self.sensor_readings = Some(Arc::new(tokio::sync::Mutex::new(PostedLog::default())));
        self
    }

    /// The same state, taking posted sensor readings when `enabled`.
    pub fn posting_when(self, enabled: bool) -> Self {
        if enabled {
            self.posting()
        } else {
            self
        }
    }

    /// The same state, able to wake the weather fetch schedule.
    // @spec SYNC-SCHED-008
    pub fn waking(mut self, waker: Waker) -> Self {
        self.weather_waker = Some(waker);
        self
    }

    pub fn new(
        pool: sqlx::PgPool,
        sync_service: DataSyncService,
        openrouter: Option<OpenRouterClient>,
        mcp_enabled: bool,
        clock: Clock,
    ) -> Self {
        Self {
            pool,
            rules_engine: Arc::new(RulesEngine::new()),
            sync_service: Arc::new(RwLock::new(sync_service)),
            openrouter: openrouter.map(Arc::new),
            climate_cache: Arc::new(RwLock::new(None)),
            crossing_memo: Arc::new(RwLock::new(None)),
            mcp_enabled,
            clock,
            weather_waker: None,
            weather: None,
            sensor_readings: None,
        }
    }
}
