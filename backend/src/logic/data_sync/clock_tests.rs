//! Data sync judges at the app clock's "now".

use super::*;
use chrono::TimeZone;

// @spec SYNC-REFRESH-009, SYNC-FAIL-010
#[tokio::test]
async fn a_frozen_clock_stamps_the_summary_at_its_instant() {
    let at = Utc.with_ymd_and_hms(2026, 3, 8, 12, 0, 0).unwrap();
    let mut config = crate::logic::data_sync::test_env::config(&[]);
    config.clock = Clock::Frozen {
        at,
        zone: chrono_tz::America::New_York,
    };
    let pool = crate::logic::data_sync::test_env::unreachable_pool();
    // The record cannot be read: the summary is empty, stamped at the clock's now.
    let summary = DataSyncService::initialize(&config, pool)
        .await
        .get_or_refresh()
        .await
        .unwrap();
    assert_eq!(summary.last_updated, Some(at));
    assert!(summary.current.is_none());
}
