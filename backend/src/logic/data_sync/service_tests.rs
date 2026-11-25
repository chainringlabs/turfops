//! The I/O half: when the service contacts its sources, and what it reports.

use super::test_env::*;
use super::test_support::{eastern, forecast, forecast_day};
use super::DataSyncService;
use crate::logic::weather_sync::forecast::ForecastSlot;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

/// A Home Assistant stub answering 200, and the service using it, with a forecast held
/// by the schedule.
async fn stubbed() -> (Stub, DataSyncService) {
    let ha = stub(200, HA_STATE).await;
    let held = ForecastSlot::holding(forecast(vec![forecast_day(
        eastern().today(),
        24,
        70.0,
        50.0,
    )]));
    let service = service(
        &config_of(&with_home_assistant(&ha.url)),
        unreachable_pool(),
    )
    .await
    .with_forecast(held);
    (ha, service)
}

// @spec SYNC-REFRESH-007
#[tokio::test]
async fn startup_sets_up_clients_without_contacting_a_source() {
    let (ha, _service) = stubbed().await;
    assert_eq!(ha.count(), 0);

    // Sources that are down do not delay startup.
    let down = with_home_assistant(UNREACHABLE);
    let started = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        DataSyncService::initialize(&config_of(&down), unreachable_pool()),
    )
    .await;
    assert!(started.is_ok(), "startup waited on a source");
}

// @spec SYNC-REFRESH-001
#[tokio::test]
async fn an_idle_server_contacts_no_source_until_a_request_needs_conditions() {
    let (ha, service) = stubbed().await;
    let state = app_state(unreachable_pool(), service, false);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(ha.count(), 0, "no poller");

    state
        .sync_service
        .write()
        .await
        .get_or_refresh()
        .await
        .unwrap();
    assert!(ha.count() > 0, "read while serving");
}

// @spec SYNC-REFRESH-003
#[tokio::test]
async fn a_fresh_summary_is_returned_without_contacting_any_source() {
    let (ha, mut service) = stubbed().await;
    let first = service.get_or_refresh().await.unwrap();
    let calls = ha.count();
    assert!(calls > 0);

    let second = service.get_or_refresh().await.unwrap();
    assert_eq!(ha.count(), calls);
    assert_eq!(second.last_updated, first.last_updated);
}

// @spec SYNC-SCHED-020, SYNC-FAIL-006
#[tokio::test]
async fn the_summary_carries_the_forecast_the_schedule_holds_and_fetches_none() {
    let (ha, mut service) = stubbed().await;
    let summary = service.get_or_refresh().await.unwrap();
    assert_eq!(
        summary.forecast.unwrap().daily_summary[0].date,
        eastern().today()
    );
    let calls = ha.count();

    // A sensor refresh reads Home Assistant again; the forecast is never fetched here.
    service.state.age_sensors(Duration::minutes(6));
    let summary = service.get_or_refresh().await.unwrap();
    assert!(ha.count() > calls, "sensors re-read");
    assert!(
        summary.forecast.is_some(),
        "the held forecast is still served"
    );
}

// @spec SYNC-REFRESH-006
#[tokio::test]
async fn simultaneous_requests_share_one_read_of_each_source() {
    let (ha, service) = stubbed().await;
    let shared = Arc::new(RwLock::new(service));
    let request = || {
        let shared = shared.clone();
        async move { shared.write().await.get_or_refresh().await.unwrap() }
    };
    let (a, b, c) = tokio::join!(request(), request(), request());
    // One refresh: Home Assistant's two entities.
    assert_eq!(ha.count(), 2, "{:?}", ha.requests());
    assert_eq!(a.last_updated, b.last_updated);
    assert_eq!(b.last_updated, c.last_updated);
}

// @spec SYNC-CONN-001, SYNC-CONN-008
#[tokio::test]
async fn connection_status_reports_each_source_by_its_probe_and_no_forecast_flag() {
    let (ha, service) = stubbed().await;
    let status = service.check_connections().await;
    assert!(!status.soildata, "the lake is not mounted");
    assert!(status.homeassistant);
    assert!(ha
        .requests()
        .iter()
        .any(|r| r.starts_with("GET /api/ ") && r.ends_with(&format!("Bearer {HA_TOKEN}"))));
    let json = serde_json::to_value(&status).unwrap();
    let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["homeassistant", "soildata"]);
}

// @spec SYNC-CONN-001
#[tokio::test]
async fn an_unconfigured_or_failing_source_reports_false() {
    let unconfigured = service(&config(&[]), unreachable_pool()).await;
    assert!(!unconfigured.check_connections().await.homeassistant);

    let ha = stub(401, "{}").await;
    let failing = service(
        &config_of(&with_home_assistant(&ha.url)),
        unreachable_pool(),
    )
    .await;
    assert!(!failing.check_connections().await.homeassistant);

    let down = service(
        &config_of(&with_home_assistant(UNREACHABLE)),
        unreachable_pool(),
    )
    .await;
    let status = down.check_connections().await;
    assert!(!status.soildata && !status.homeassistant);
}

// @spec SYNC-CONN-002
#[tokio::test]
async fn a_connection_check_leaves_the_summary_and_its_timers_alone() {
    let (_ha, mut service) = stubbed().await;
    service.check_connections().await;
    let now = Utc::now();
    assert!(service.state.due(now), "a probe is not a refresh");
    assert_eq!(service.summary().last_updated, None);

    service.get_or_refresh().await.unwrap();
    service.state.age_sensors(Duration::minutes(2));
    let before = service.summary();
    let due = service.state.due(Utc::now());
    *service.probes.lock().unwrap() = Default::default();
    service.check_connections().await;
    assert_eq!(service.state.due(Utc::now()), due);
    let after = service.summary();
    assert_eq!(after.last_updated, before.last_updated);
    assert_eq!(after.forecast.is_some(), before.forecast.is_some());
}

// @spec SYNC-FAIL-001
#[tokio::test]
async fn a_refresh_with_every_source_failing_still_returns_a_summary() {
    let ha = stub(500, "{}").await;
    let service = service(
        &config_of(&with_home_assistant(&ha.url)),
        unreachable_pool(),
    )
    .await;
    let state = app_state(unreachable_pool(), service, false);

    let axum::Json(summary) =
        crate::api::environmental::get_environmental(axum::extract::State(state.clone()))
            .await
            .expect("GET /environmental absorbs source errors");
    assert!(summary.current.is_some() && summary.last_updated.is_some());
    assert!(ha.count() > 0, "every source was tried");

    let axum::Json(forced) =
        crate::api::environmental::refresh_environmental(axum::extract::State(state))
            .await
            .expect("POST /environmental/refresh absorbs source errors");
    assert!(forced.current.is_some());
    assert!(forced.forecast.is_none());
}
