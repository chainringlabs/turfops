//! Passes of the fetch schedule against PostgreSQL, with every outside source stubbed
//! or unreachable.

use super::fetcher::Clients;
use super::schedule::{Pass, Schedule};
use crate::datasources::grid::GridClient;
use crate::datasources::open_meteo::OpenMeteoClient;
use crate::datasources::outside::Outside;
use crate::datasources::scan::ScanClient;
use crate::datasources::uscrn::UscrnClient;
use crate::db::weather_queries::{catalog, fetch_state, lawn, plan, sources};
use crate::models::weather_record::*;
use crate::models::{GrassType, LawnLocation};
use crate::test_support::stub_http_sequence;
use chrono::{DateTime, Duration, TimeZone, Utc};
use sqlx::PgPool;
use std::sync::Arc;

const NOWHERE: &str = "http://127.0.0.1:1";

pub(super) fn unreachable() -> Clients {
    let outside = Arc::new(Outside::unpaced());
    Clients {
        uscrn: UscrnClient::new(outside.clone()).with_base(NOWHERE),
        scan: ScanClient::new(outside.clone()).with_base(NOWHERE),
        grid: GridClient::new(outside.clone()).with_url(NOWHERE),
        open_meteo: OpenMeteoClient::new(outside).with_urls(NOWHERE, NOWHERE),
    }
}

/// Minutes after 16:00 UTC today. The passes run on the real date because a source's
/// wanted span starts from the day it came into use.
pub(super) fn at(minutes: i64) -> DateTime<Utc> {
    let today = Utc::now().date_naive();
    Utc.from_utc_datetime(&today.and_hms_opt(16, 0, 0).unwrap()) + Duration::minutes(minutes)
}

fn station(network: Network, id: &str, miles_north: f64, feet: f64) -> Station {
    Station {
        network,
        station_id: id.into(),
        name: format!("Station {id}"),
        state: "PA".into(),
        latitude: 39.86 + miles_north / 69.09,
        longitude: -75.79,
        elevation_ft: Some(feet),
        inputs: vec![Input::SoilTemp5, Input::SoilTemp10],
        first_year: Some(2006),
        last_year: None,
        locator: Some(format!("PA_{id}")),
        utc_offset_hours: None,
    }
}

/// A lawn near Avondale with both networks' station lists freshly loaded: a USCRN
/// station 20 miles off, 2,000 feet up.
pub(super) async fn located(pool: &PgPool) {
    let mut profile = crate::logic::rules::test_fixtures::profile(GrassType::TallFescue);
    profile.id = None;
    profile.location = Some(LawnLocation::at(39.86, -75.79, None).unwrap());
    crate::db::queries::create_lawn_profile(pool, &profile)
        .await
        .unwrap();
    let uscrn = [station(Network::Uscrn, "a", 20.0, 2000.0)];
    let scan = [station(Network::Scan, "s", 400.0, 400.0)];
    catalog::replace_network(pool, Network::Uscrn, &uscrn)
        .await
        .unwrap();
    catalog::replace_network(pool, Network::Scan, &scan)
        .await
        .unwrap();
}

pub(super) async fn state_of(pool: &PgPool, key: &str) -> FetchState {
    let source = sources::all(pool)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.key == key)
        .expect(key);
    let states = fetch_state::all(pool).await.unwrap();
    states
        .into_iter()
        .find(|s| s.source_id == source.id)
        .expect(key)
}

const STATION: &str = "station:Uscrn:a";
const PRISM: &str = "grid:prism:39.8600,-75.7900";
const ANALYSIS: &str = "analysis:39.8600,-75.7900";

// @spec SYNC-SCHED-011, WX-REC-PLAN-011
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_no_location_a_pass_plans_nothing_and_asks_nobody(pool: PgPool) {
    let (mut schedule, _, _) = Schedule::new(pool.clone(), unreachable());
    assert_eq!(schedule.pass(at(0)).await.unwrap(), Pass::Rest);
    assert!(plan::read(&pool).await.unwrap().is_empty());
    assert!(fetch_state::all(&pool).await.unwrap().is_empty());
}

// @spec SYNC-SCHED-001, WX-REC-PLAN-002, SYNC-SCHED-006, SYNC-SCHED-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_pass_builds_the_plan_and_one_failing_source_does_not_stop_the_others(pool: PgPool) {
    located(&pool).await;
    let (mut schedule, _, _) = Schedule::new(pool.clone(), unreachable());
    schedule.pass(at(0)).await.unwrap();

    let planned = plan::read(&pool).await.unwrap();
    let soil: Vec<_> = planned
        .iter()
        .filter(|p| p.input == Input::SoilTemp10)
        .map(|p| p.source.key.as_str())
        .collect();
    assert_eq!(soil, vec![STATION, "estimate:39.8600,-75.7900"]);
    for key in [STATION, PRISM, ANALYSIS] {
        let state = state_of(&pool, key).await;
        assert_eq!(state.last_failure, Some(at(0)), "{key} was tried");
        assert_eq!(state.held, None);
    }
    let minute = lawn::fetch_minute(&pool, 99).await.unwrap().unwrap();
    assert!(minute < 60, "a minute was chosen and kept");
}

// @spec SYNC-SCHED-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_failed_fetch_waits_five_minutes_before_it_is_tried_again(pool: PgPool) {
    located(&pool).await;
    let (mut schedule, _, _) = Schedule::new(pool.clone(), unreachable());
    schedule.pass(at(0)).await.unwrap();
    schedule.pass(at(1)).await.unwrap();
    schedule.pass(at(4)).await.unwrap();
    assert_eq!(state_of(&pool, PRISM).await.last_attempt, Some(at(0)));
    schedule.pass(at(5)).await.unwrap();
    assert_eq!(state_of(&pool, PRISM).await.last_attempt, Some(at(5)));
    schedule.pass(at(19)).await.unwrap();
    assert_eq!(
        state_of(&pool, PRISM).await.last_attempt,
        Some(at(5)),
        "then fifteen"
    );
    schedule.pass(at(20)).await.unwrap();
    assert_eq!(state_of(&pool, PRISM).await.last_attempt, Some(at(20)));
}

// @spec SYNC-SCHED-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_every_source_failing_the_passes_come_to_rest(pool: PgPool) {
    located(&pool).await;
    let (mut schedule, _, _) = Schedule::new(pool.clone(), unreachable());
    let mut passes = 0;
    while schedule.pass(at(0)).await.unwrap() == Pass::Again {
        passes += 1;
        assert!(passes < 20, "backfill must not spin while its sources fail");
    }
}

/// The grid's answer: the two days before today, and today not yet published.
fn grid_body() -> &'static str {
    let today = at(0).date_naive();
    let day = |back: i64| today - Duration::days(back);
    let body = format!(
        r#"{{"meta":{{"lon": -75.79166, "lat": 39.83333}},
        "data":[["{}",80.0,60.0,0],["{}",82.0,61.0,0.25],["{}",-999,-999,-999]]}}"#,
        day(2),
        day(1),
        day(0)
    );
    Box::leak(body.into_boxed_str())
}

// @spec SYNC-SCHED-002, SYNC-SCHED-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_pass_writes_what_a_reachable_source_returns(pool: PgPool) {
    located(&pool).await;
    let (base, mut requests) = stub_http_sequence(vec![(200, grid_body())]).await;
    let mut clients = unreachable();
    clients.grid = GridClient::new(Arc::new(Outside::unpaced())).with_url(&base);
    let (mut schedule, _, _) = Schedule::new(pool.clone(), clients);
    schedule.pass(at(0)).await.unwrap();
    assert!(requests.recv().await.is_some());
    let prism = state_of(&pool, PRISM).await;
    assert_eq!(prism.last_success, Some(at(0)));
    assert_eq!(
        prism.held.unwrap().1,
        at(0).date_naive() - Duration::days(1)
    );
}

// @spec WX-REC-PLAN-013, WX-REC-PLAN-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn when_the_lawns_elevation_becomes_known_the_plan_is_rebuilt(pool: PgPool) {
    located(&pool).await;
    let (mut schedule, _, _) = Schedule::new(pool.clone(), unreachable());
    schedule.pass(at(0)).await.unwrap();
    let has_station = |planned: &[plan::Placed]| planned.iter().any(|p| p.source.key == STATION);
    assert!(
        has_station(&plan::read(&pool).await.unwrap()),
        "no elevation, no guard"
    );

    // The lawn is at 400 feet; the station is at 2,000.
    lawn::set_elevation_ft(&pool, 400.0).await.unwrap();
    schedule.pass(at(1)).await.unwrap();
    assert!(!has_station(&plan::read(&pool).await.unwrap()));
}

// @spec SYNC-SCHED-010
#[sqlx::test(migrations = "src/db/migrations")]
async fn once_told_to_stop_no_further_fetch_is_started(pool: PgPool) {
    located(&pool).await;
    let (mut schedule, _, stopper) = Schedule::new(pool.clone(), unreachable());
    stopper.stop();
    schedule.pass(at(0)).await.unwrap();
    let attempted = fetch_state::all(&pool)
        .await
        .unwrap()
        .iter()
        .any(|s| s.last_attempt.is_some());
    assert!(!attempted);
    // And the loop ends.
    tokio::time::timeout(std::time::Duration::from_secs(5), schedule.run())
        .await
        .expect("the schedule ends when stopped");
}

/// The first passes of a new installation against the real services, into the test
/// database. Not part of the suite:
/// `cargo test --features db-tests live_schedule -- --ignored`.
#[sqlx::test(migrations = "src/db/migrations")]
#[ignore = "calls NOAA, USDA, RCC-ACIS and Open-Meteo"]
async fn live_schedule_fills_a_new_lawn(pool: PgPool) {
    // Shows the schedule's own log lines with `--nocapture`.
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let mut profile = crate::logic::rules::test_fixtures::profile(GrassType::TallFescue);
    profile.id = None;
    profile.location = Some(LawnLocation::at(39.86, -75.79, None).unwrap());
    crate::db::queries::create_lawn_profile(&pool, &profile)
        .await
        .unwrap();
    let (mut schedule, _, _) = Schedule::new(pool.clone(), Clients::new());

    // Catalogs, then the plan and the present, then some history.
    for _ in 0..6 {
        schedule.pass(Utc::now()).await.unwrap();
    }
    let stations = catalog::stations(&pool).await.unwrap();
    assert!(stations.len() > 200, "{} stations", stations.len());
    let planned = plan::read(&pool).await.unwrap();
    let soil = planned
        .iter()
        .find(|p| p.input == Input::SoilTemp10)
        .unwrap();
    assert_eq!(
        soil.source.kind,
        SourceKind::Station,
        "{}",
        soil.source.name
    );
    assert!(lawn::elevation_ft(&pool).await.unwrap().is_some());

    let all = sources::all(&pool).await.unwrap();
    let states = fetch_state::all(&pool).await.unwrap();
    for state in &states {
        let source = all.iter().find(|s| s.id == state.source_id).unwrap();
        println!(
            "{:<45} {:<17} held {:?} failure {:?}",
            source.key, state.product, state.held, state.last_error
        );
    }
    let failed: Vec<_> = states.iter().filter(|s| s.last_failure.is_some()).collect();
    assert!(failed.is_empty(), "{failed:?}");
    let calibrating = all
        .iter()
        .filter(|s| s.role == SourceRole::Calibration)
        .count();
    // The basis station is waited for, so only the grid at its point is held yet.
    assert!(calibrating >= 1, "{calibrating} calibration sources");
}
