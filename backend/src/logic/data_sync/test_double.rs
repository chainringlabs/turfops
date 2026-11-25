//! A `DataSyncService` for tests that need an `AppState`: no Home Assistant, no forecast,
//! and the weather lake only when the test supplies a client.

use super::DataSyncService;
use crate::clock::Clock;
use crate::datasources::WeatherLakeClient;
use crate::logic::weather_sync::forecast::ForecastSlot;
use sqlx::PgPool;

impl DataSyncService {
    /// A service with only the lake client given (or none, as when the lake is not
    /// configured) and no external connections.
    pub(crate) fn with_lake_only(
        pool: PgPool,
        weather_client: Option<WeatherLakeClient>,
        clock: Clock,
    ) -> Self {
        Self {
            pool,
            weather_client,
            forecast: ForecastSlot::new(),
            waker: None,
            kept: Default::default(),
            refreshed_at: None,
            clock,
            lake_summary: true,
            pinned: None,
        }
    }
}
