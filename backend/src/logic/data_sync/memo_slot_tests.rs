//! The climate-record slot in `AppState`: one per process, empty at startup, shared.

use super::test_env::*;
use std::sync::Arc;

// @spec SYNC-MEMO-001
#[tokio::test]
async fn one_climate_slot_starts_empty_and_is_shared_by_every_caller() {
    let state = offline_state(unreachable_pool()).await;
    assert!(
        state.climate_cache.read().await.is_none(),
        "empty at startup"
    );
    let elsewhere = state.clone();
    assert!(Arc::ptr_eq(&state.climate_cache, &elsewhere.climate_cache));
}
