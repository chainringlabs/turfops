//! One fetch at a time against PostgreSQL, with each outside source stubbed.

use super::fetcher::{Clients, Fetcher};
use crate::datasources::cadence::Product;
use crate::datasources::grid::GridClient;
use crate::datasources::open_meteo::OpenMeteoClient;
use crate::datasources::outside::Outside;
use crate::datasources::scan::ScanClient;
use crate::datasources::uscrn::UscrnClient;
use crate::db::weather_queries::rows;
use crate::db::weather_queries::{catalog, fetch_state, lawn};
use crate::logic::weather_record::due::{Fetch, Purpose};
use crate::logic::weather_record::lab::{
    d, noon, Lab, ANALYSIS as ANALYSIS_SOURCE, NRCC, PRISM, STATION,
};
use crate::models::weather_record::Network;
use crate::models::weather_record::{FetchState, Input};
use crate::test_support::{stub_http, stub_http_sequence};
use chrono::NaiveDate;
use chrono::TimeZone;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::sync::Arc;

const NOWHERE: &str = "http://127.0.0.1:1";

/// Clients that reach nothing, with one pointed at a stub by the test.
fn clients(aim: impl FnOnce(&mut Clients)) -> Clients {
    let outside = Arc::new(Outside::unpaced());
    let mut clients = Clients {
        uscrn: UscrnClient::new(outside.clone()).with_base(NOWHERE),
        scan: ScanClient::new(outside.clone()).with_base(NOWHERE),
        grid: GridClient::new(outside.clone()).with_url(NOWHERE),
        open_meteo: OpenMeteoClient::new(outside.clone()).with_urls(NOWHERE, NOWHERE),
        nws: crate::datasources::nws::NwsClient::new(outside).with_base_url(NOWHERE),
        owm: crate::datasources::OpenWeatherMapClient::at(NOWHERE),
    };
    aim(&mut clients);
    clients
}

fn outside() -> Arc<Outside> {
    Arc::new(Outside::unpaced())
}

/// A lawn whose sources are registered and hold nothing.
async fn new_lawn(pool: PgPool) -> Lab {
    let lab = Lab::new(pool).await;
    sqlx::query("DELETE FROM weather_fetch_state")
        .execute(&lab.pool)
        .await
        .unwrap();
    lab.record.register_wanted(d(20)).await.unwrap();
    lab
}

async fn state(lab: &Lab, key: &str) -> FetchState {
    let id = lab.source(key).await.id;
    let all = fetch_state::all(&lab.pool).await.unwrap();
    all.into_iter().find(|s| s.source_id == id).unwrap()
}

async fn fetch(lab: &Lab, key: &str, product: Product, span: (NaiveDate, NaiveDate)) -> Fetch {
    Fetch {
        source: lab.source(key).await,
        product,
        span,
        purpose: Purpose::Current,
    }
}

const GRID: &str = r#"{"meta":{"lon": -75.79166, "lat": 39.83333},
"data":[["2026-07-17",80.0,60.0,0],["2026-07-18",82.0,61.0,0.25],["2026-07-19",-999,-999,-999]]}"#;

// @spec SYNC-SCHED-002, WX-REC-FILL-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_grid_fetch_writes_its_days_and_holds_what_the_grid_returned(pool: PgPool) {
    let lab = new_lawn(pool).await;
    let base = stub_http(200, GRID).await;
    let fetcher = Fetcher::new(
        lab.pool.clone(),
        clients(|c| c.grid = GridClient::new(outside()).with_url(&base)),
    );
    let now = noon(20).at.now;
    let wanted = fetch(&lab, PRISM, Product::PrismDays, (d(17), d(20))).await;
    assert_eq!(fetcher.run(&wanted, now).await.unwrap(), 2, "rows written");

    let held = rows::daily(&lab.pool, wanted.source.id, d(1), d(31))
        .await
        .unwrap();
    assert_eq!(held.len(), 2);
    assert_eq!(
        (held[1].air_max_f, held[1].air_min_f),
        (Some(82.0), Some(61.0))
    );
    let state = state(&lab, PRISM).await;
    assert_eq!(
        state.held,
        Some((d(17), d(18))),
        "the 19th is not published yet"
    );
    assert_eq!((state.last_success, state.last_failure), (Some(now), None));
}

// @spec SYNC-SCHED-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_fetch_that_brings_nothing_new_is_a_success_that_holds_nothing_more(pool: PgPool) {
    let lab = new_lawn(pool).await;
    let empty = r#"{"meta":{"lon": -75.79, "lat": 39.83},"data":[["2026-07-20",-999,-999,-999]]}"#;
    let base = stub_http(200, empty).await;
    let fetcher = Fetcher::new(
        lab.pool.clone(),
        clients(|c| c.grid = GridClient::new(outside()).with_url(&base)),
    );
    let now = noon(20).at.now;
    let wanted = fetch(&lab, NRCC, Product::NrccDays, (d(20), d(20))).await;
    fetcher.run(&wanted, now).await.unwrap();
    let state = state(&lab, NRCC).await;
    assert_eq!((state.held, state.last_success), (None, Some(now)));
}

// @spec SYNC-SCHED-006, WX-REC-FILL-003, WX-REC-FAIL-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_failed_fetch_is_recorded_against_its_source_and_nothing_held_changes(pool: PgPool) {
    let lab = Lab::new(pool).await;
    let base = stub_http(500, "upstream exploded").await;
    let fetcher = Fetcher::new(
        lab.pool.clone(),
        clients(|c| c.grid = GridClient::new(outside()).with_url(&base)),
    );
    let before = state(&lab, PRISM).await;
    let now = noon(20).at.now;
    let wanted = fetch(&lab, PRISM, Product::PrismDays, (d(17), d(20))).await;
    assert!(fetcher.run(&wanted, now).await.is_err());
    let after = state(&lab, PRISM).await;
    assert_eq!(after.held, before.held);
    assert_eq!((after.last_failure, after.last_success), (Some(now), None));
    let said = after.last_error.unwrap();
    assert!(said.contains("temperature and rain grid"), "{said}");
    assert!(
        !said.contains("exploded") && !said.contains("127.0.0.1"),
        "{said}"
    );
}

const ANALYSIS: &str = r#"{"latitude":39.86,"longitude":-75.8,"elevation":136.0,
"hourly_units":{"time":"unixtime","temperature_2m":"°F","relative_humidity_2m":"%","precipitation":"mm"},
"hourly":{"time":[1795132800,1795136400,1795140000,1795143600],
"temperature_2m":[50.1,49.0,45.4,44.0],"relative_humidity_2m":[44,43,52,55],
"precipitation":[0.0,0.0,1.2,0.0]}}"#;

// @spec SYNC-SCHED-002, WX-REC-PLAN-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn an_analysis_fetch_writes_its_hours_and_the_lawns_elevation(pool: PgPool) {
    let lab = new_lawn(pool).await;
    let (base, mut requests) = stub_http_sequence(vec![(200, ANALYSIS)]).await;
    let fetcher = Fetcher::new(
        lab.pool.clone(),
        clients(|c| c.open_meteo = OpenMeteoClient::new(outside()).with_urls(&base, &base)),
    );
    assert_eq!(lawn::elevation_ft(&lab.pool).await.unwrap(), None);
    let now = Utc.with_ymd_and_hms(2026, 11, 20, 3, 30, 0).unwrap();
    let day = now.date_naive();
    let wanted = fetch(
        &lab,
        ANALYSIS_SOURCE,
        Product::AnalysisHours,
        (day - Duration::days(9), day),
    )
    .await;
    fetcher.run(&wanted, now).await.unwrap();

    assert!(requests.recv().await.unwrap().contains("past_days=10&"));
    let from = now - Duration::days(1);
    let hours = rows::hourly(&lab.pool, wanted.source.id, from, now)
        .await
        .unwrap();
    assert_eq!(hours.len(), 3, "the hours that have ended");
    assert_eq!(hours[1].precip_mm, Some(1.2));
    let feet = lawn::elevation_ft(&lab.pool).await.unwrap().unwrap();
    assert!((feet - 446.19).abs() < 0.01);
}

const HOUR: &str = "03761 20260118 2200 20260118 1700  2.622  -75.79   39.86    -0.9    -0.5    -0.1    -0.9     0.8      6 0     18 0      0 0 C    -0.9 0    -0.8 0    -1.2 0    88 0   0.357   0.348   0.343   0.350   0.301     1.3     1.5     1.5     2.8     3.8\n";

async fn with_locator(lab: &Lab) {
    sqlx::query("UPDATE weather_station SET locator = 'PA_Avondale_2_N'")
        .execute(&lab.pool)
        .await
        .unwrap();
}

fn jan(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 1, day).unwrap()
}

// @spec SYNC-SCHED-002, WX-REC-FILL-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_station_fetch_reads_each_years_file_and_keeps_the_span_asked_for(pool: PgPool) {
    let lab = new_lawn(pool).await;
    with_locator(&lab).await;
    let (base, mut requests) = stub_http_sequence(vec![(404, ""), (200, HOUR), (304, "")]).await;
    let fetcher = Fetcher::new(
        lab.pool.clone(),
        clients(|c| c.uscrn = UscrnClient::new(outside()).with_base(&base)),
    );
    let now = Utc.with_ymd_and_hms(2026, 1, 20, 12, 0, 0).unwrap();
    let dec = NaiveDate::from_ymd_opt(2025, 12, 30).unwrap();
    let wanted = fetch(&lab, STATION, Product::StationHours, (dec, jan(20))).await;
    fetcher.run(&wanted, now).await.unwrap();

    assert_eq!(
        requests.recv().await.unwrap(),
        "GET /hourly02/2025/CRNH0203-2025-PA_Avondale_2_N.txt HTTP/1.1"
    );
    assert!(requests.recv().await.unwrap().contains("/hourly02/2026/"));
    let start = Utc.with_ymd_and_hms(2026, 1, 18, 0, 0, 0).unwrap();
    let held = rows::hourly(&lab.pool, wanted.source.id, start, now)
        .await
        .unwrap();
    assert_eq!(held.len(), 1);
    assert_eq!(
        held[0].observed_at,
        Utc.with_ymd_and_hms(2026, 1, 18, 21, 0, 0).unwrap()
    );
    let after = state(&lab, STATION).await;
    assert_eq!(
        after.held,
        Some((dec, jan(20))),
        "a year with no file is held as empty"
    );
    // What the station measures is learned from its rows.
    let learned = &catalog::stations(&lab.pool).await.unwrap()[0].1.inputs;
    assert!(learned.contains(&Input::SoilMoisture100) && learned.contains(&Input::AirTemp));
    assert_eq!(learned.len(), 13, "{learned:?}");

    // An hour later the file has not changed: a success that writes nothing.
    let later = now + Duration::hours(1);
    let again = fetch(&lab, STATION, Product::StationHours, (jan(17), jan(20))).await;
    fetcher.run(&again, later).await.unwrap();
    assert_eq!(state(&lab, STATION).await.last_success, Some(later));
}

const STATIONS: &str = "WBAN\tCOUNTRY\tSTATE\tLOCATION\tVECTOR\tNAME\tLATITUDE\tLONGITUDE\tELEVATION\tSTATUS\tCOMMISSIONING\tCLOSING\tOPERATION\tPAIRING\tNETWORK\tSTATION_ID
03761\tUS\tPA\tAvondale\t2 N\tStroud Water Research Center\t39.85\t-75.78\t400\tCommissioned\t2006-09-29 00:00:00.0\t\tOperational\t\tUSCRN\t1123
23801\tUS\tAL\tTroy\t2 W\tTroy\t31.79\t-86\t472\tExperimental\t\t\tOperational\t\tAlabama-USRCRN\t1552
";

// @spec WX-REC-FILL-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_catalog_refresh_replaces_the_networks_station_list(pool: PgPool) {
    let base = stub_http(200, STATIONS).await;
    let fetcher = Fetcher::new(
        pool.clone(),
        clients(|c| c.uscrn = UscrnClient::new(outside()).with_base(&base)),
    );
    fetcher.refresh_catalog(Network::Uscrn).await.unwrap();
    let held = catalog::stations(&pool).await.unwrap();
    assert_eq!(held.len(), 2);
    let record = crate::logic::weather_record::record::WeatherRecord::new(pool);
    let due = record.catalogs_due(Utc::now()).await.unwrap();
    assert_eq!(due, vec![Network::Scan], "USCRN is fresh");
}
