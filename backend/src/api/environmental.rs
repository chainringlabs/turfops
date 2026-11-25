use crate::error::TurfOpsError;
use crate::logic::data_sync::DataSyncService;
use crate::models::EnvironmentalSummary;
use crate::state::AppState;
use axum::extract::State;
use axum::Json;

// @spec WX-VIEW-API-001
/// GET /api/v1/environmental: data sync's summary.
pub async fn get_environmental(
    State(state): State<AppState>,
) -> Result<Json<EnvironmentalSummary>, TurfOpsError> {
    let mut service = state.sync_service.write().await;
    let summary = service.get_or_refresh().await?;
    Ok(Json(summary))
}

// @spec WX-VIEW-API-001, SYNC-SCHED-009
/// POST /api/v1/environmental/refresh: asks the fetch schedule for a refresh pass, waits
/// up to 20 seconds for it without holding the service, and returns the summary after.
pub async fn refresh_environmental(
    State(state): State<AppState>,
) -> Result<Json<EnvironmentalSummary>, TurfOpsError> {
    let asked = state.sync_service.write().await.ask_refresh();
    if let Some((waker, seen)) = asked {
        waker
            .pass_after(seen, DataSyncService::refresh_wait())
            .await;
    }
    let mut service = state.sync_service.write().await;
    service.discard_summary();
    let summary = service.get_or_refresh().await?;
    Ok(Json(summary))
}
