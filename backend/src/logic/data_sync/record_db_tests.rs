//! The summary and source status from the weather record (`--features db-tests`).

use super::test_env::*;
use super::DataSyncService;
use crate::db::weather_queries::{catalog, fetch_state, rows, sources};
use crate::logic::weather_sync::schedule::Schedule;
use crate::models::weather_record::*;
use crate::models::{BasisStanding, GrassType, LawnLocation};
use chrono::{DurationRound, Utc};
use sqlx::PgPool;

const ANALYSIS: &str = "analysis:39.8600,-75.7900";

/// A lawn near Avondale whose plan has a USCRN station 20 miles off for soil and the
/// hourly analysis for air, humidity and rain.
pub(crate) async fn located(pool: &PgPool) {
    let mut profile = crate::logic::rules::test_fixtures::profile(GrassType::TallFescue);
    profile.id = None;
    profile.location = Some(LawnLocation::at(39.86, -75.79, None).unwrap());
    crate::db::queries::create_lawn_profile(pool, &profile)
        .await
        .unwrap();
    let station = Station {
        network: Network::Uscrn,
        station_id: "a".into(),
        name: "PA Avondale 2 N".into(),
        state: "PA".into(),
        latitude: 39.86 + 20.0 / 69.09,
        longitude: -75.79,
        elevation_ft: Some(400.0),
        inputs: vec![Input::SoilTemp10, Input::SoilMoisture10],
        first_year: Some(2006),
        last_year: None,
        locator: Some("PA_a".into()),
        utc_offset_hours: None,
    };
    catalog::replace_network(pool, Network::Uscrn, &[station])
        .await
        .unwrap();
    catalog::replace_network(pool, Network::Scan, &[])
        .await
        .unwrap();
    crate::logic::weather_sync::planning::rebuild(pool)
        .await
        .unwrap();
}

async fn source(pool: &PgPool, key: &str) -> Source {
    sources::all(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.key == key)
        .unwrap_or_else(|| panic!("{key} is not a source"))
}

/// The analysis reported air temperature and humidity for the last three hours.
async fn air_reported(pool: &PgPool) {
    let analysis = source(pool, ANALYSIS).await;
    let this_hour = Utc::now()
        .duration_trunc(chrono::Duration::hours(1))
        .unwrap();
    let hours: Vec<HourlyRow> = (1..=3)
        .map(|back| HourlyRow {
            observed_at: this_hour - chrono::Duration::hours(back),
            air_temp_f: Some(40.0 + back as f64),
            rh_pct: Some(60.0),
            precip_mm: Some(0.0),
            ..Default::default()
        })
        .collect();
    let mut conn = pool.acquire().await.unwrap();
    rows::write_hourly(&mut conn, analysis.id, &hours)
        .await
        .unwrap();
}

async fn service(pool: &PgPool) -> DataSyncService {
    DataSyncService::initialize(&config(&[]), pool.clone()).await
}

// @spec SYNC-MERGE-007, SYNC-MERGE-010, SYNC-MERGE-011, WX-VIEW-API-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_summary_is_assembled_from_the_record_with_each_basis(pool: PgPool) {
    located(&pool).await;
    air_reported(&pool).await;
    let summary = service(&pool).await.get_or_refresh().await.unwrap();

    let current = summary.current.as_ref().unwrap();
    assert_eq!(current.ambient_temp_f, Some(41.0), "the newest hour");
    assert_eq!(current.humidity_percent, Some(60.0));
    assert_eq!(
        current.soil_temp_10_f, None,
        "the station has reported nothing"
    );
    let air = summary
        .current_bases
        .iter()
        .find(|b| b.input == "air_temp")
        .unwrap();
    assert_eq!(air.standing, BasisStanding::InUse);
    let basis = air.basis.as_ref().unwrap();
    assert!(basis.approximate, "the hourly analysis is approximate");
    let soil = summary
        .current_bases
        .iter()
        .find(|b| b.input == "soil_temp_10")
        .unwrap();
    assert_eq!(soil.standing, BasisStanding::NoCurrentValue);
    let deep = summary
        .current_bases
        .iter()
        .find(|b| b.input == "soil_moisture_100")
        .unwrap();
    assert_eq!(deep.standing, BasisStanding::NoSource);
    assert!(summary.week_bases.contains_key("ambient_temp_7day_avg_f"));
    assert!(
        !summary.soil_stale,
        "no soil value at any age is not stale soil"
    );
}

// @spec SYNC-FAIL-011
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_no_location_the_weather_fields_are_empty(pool: PgPool) {
    let mut profile = crate::logic::rules::test_fixtures::profile(GrassType::TallFescue);
    profile.id = None;
    crate::db::queries::create_lawn_profile(&pool, &profile)
        .await
        .unwrap();
    let summary = service(&pool).await.get_or_refresh().await.unwrap();
    assert!(summary.current.is_none());
    assert!(summary.current_bases.is_empty() && summary.week_bases.is_empty());
    assert_eq!(summary.ambient_temp_7day_avg_f, None);
    assert_eq!(summary.gdd_base50_ytd, None);
    assert!(summary.last_updated.is_some());
}

// @spec SYNC-CONN-001, SYNC-CONN-002, WX-VIEW-STATUS-001, WX-VIEW-STATUS-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn source_status_is_one_entry_per_source_from_its_last_scheduled_fetch(pool: PgPool) {
    located(&pool).await;
    let sync = service(&pool).await;
    let status = sync.sources().await.unwrap();
    let analysis = status
        .iter()
        .find(|s| s.name == "Open-Meteo hourly analysis");
    let analysis = analysis.unwrap_or_else(|| panic!("{status:?}"));
    assert_eq!(analysis.ok, None, "never fetched: pending");
    assert!(analysis.inputs.contains(&"air_temp"));

    let id = source(&pool, ANALYSIS).await.id;
    // PostgreSQL keeps microseconds; Linux clocks give nanoseconds.
    let now = crate::logic::weather_sync::sensor_reading::record_instant(Utc::now());
    crate::logic::weather_record::record::WeatherRecord::new(pool.clone())
        .register_wanted(now.date_naive())
        .await
        .unwrap();
    fetch_state::failed(
        &pool,
        id,
        "analysis_hours",
        "Open-Meteo did not answer.",
        now,
    )
    .await
    .unwrap();
    let status = sync.sources().await.unwrap();
    let analysis = status
        .iter()
        .find(|s| s.name == "Open-Meteo hourly analysis")
        .unwrap();
    assert_eq!(analysis.ok, Some(false));
    assert_eq!(analysis.last_failure, Some(now));
    assert_eq!(
        analysis.last_error.as_deref(),
        Some("Open-Meteo did not answer.")
    );
    let json = serde_json::to_value(analysis).unwrap();
    for key in [
        "id",
        "name",
        "kind",
        "ok",
        "last_success",
        "last_failure",
        "last_error",
        "inputs",
        "held_from",
        "held_through",
        "wanted_from",
    ] {
        assert!(json.get(key).is_some(), "{key}: {json}");
    }
}

// @spec SYNC-SCHED-009
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_refresh_waits_for_the_schedule_s_pass_and_asks_at_most_once_a_minute(pool: PgPool) {
    located(&pool).await;
    let (schedule, waker, stopper) = Schedule::new(
        pool.clone(),
        crate::logic::weather_sync::unreachable_clients(),
    );
    let running = tokio::spawn(schedule.run());
    assert!(
        waker
            .pass_after(0, std::time::Duration::from_secs(10))
            .await,
        "the first pass runs at once"
    );
    let mut sync = service(&pool).await.with_waker(waker.clone());

    let before = waker.passes();
    sync.force_refresh().await.unwrap();
    assert!(waker.passes() > before, "the refresh waited for a pass");

    let after_first = waker.passes();
    let started = std::time::Instant::now();
    sync.force_refresh().await.unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "a second refresh inside the minute asks for nothing and waits for nothing"
    );
    assert_eq!(waker.passes(), after_first);

    stopper.stop();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), running).await;
}

// @spec WX-VIEW-API-001, SYNC-MERGE-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_endpoint_serves_the_memo_and_a_refresh_assembles_afresh(pool: PgPool) {
    use crate::api::environmental::{get_environmental, refresh_environmental};
    use axum::extract::State;
    use axum::Json;
    located(&pool).await;
    air_reported(&pool).await;
    let state = app_state(pool.clone(), service(&pool).await, false);
    let ambient = |summary: &crate::models::EnvironmentalSummary| {
        summary.current.as_ref().and_then(|c| c.ambient_temp_f)
    };

    let Json(first) = get_environmental(State(state.clone())).await.unwrap();
    assert_eq!(ambient(&first), Some(41.0));

    // A newer hour arrives; within the minute the memo is served.
    let analysis = source(&pool, ANALYSIS).await;
    let this_hour = Utc::now()
        .duration_trunc(chrono::Duration::hours(1))
        .unwrap();
    let newer = HourlyRow {
        observed_at: this_hour,
        air_temp_f: Some(50.0),
        rh_pct: Some(60.0),
        precip_mm: Some(0.0),
        ..Default::default()
    };
    let mut conn = pool.acquire().await.unwrap();
    rows::write_hourly(&mut conn, analysis.id, &[newer])
        .await
        .unwrap();
    drop(conn);
    let Json(cached) = get_environmental(State(state.clone())).await.unwrap();
    assert_eq!(ambient(&cached), Some(41.0));
    assert_eq!(cached.last_updated, first.last_updated);

    let Json(refreshed) = refresh_environmental(State(state)).await.unwrap();
    assert_eq!(ambient(&refreshed), Some(50.0), "assembled afresh");
}
