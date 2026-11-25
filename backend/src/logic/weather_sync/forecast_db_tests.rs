//! The forecast fetched by the schedule's passes, against PostgreSQL with the providers
//! stubbed.

use super::fetcher::Clients;
use super::forecast::{needs_key, status, ForecastStatus};
use super::schedule::Schedule;
use super::schedule_db_tests::{at, located, unreachable};
use crate::datasources::open_meteo::OpenMeteoClient;
use crate::datasources::outside::Outside;
use crate::datasources::OpenWeatherMapClient;
use crate::db::setup_queries::{self, Credential, Service};
use crate::models::forecast::ForecastProvider;
use crate::test_support::stub_http_seeing;
use sqlx::PgPool;
use std::sync::Arc;

/// Two Open-Meteo hours.
const OPEN_METEO: &str = r#"{"hourly_units":{"time":"unixtime","temperature_2m":"°F",
 "relative_humidity_2m":"%","apparent_temperature":"°F","precipitation":"mm",
 "precipitation_probability":"%","wind_speed_10m":"mp/h","wind_gusts_10m":"mp/h",
 "cloud_cover":"%","weather_code":"wmo code"},
"hourly":{"time":[1795348800,1795352400],"temperature_2m":[52.8,51.5],
"relative_humidity_2m":[57,68],"apparent_temperature":[49.0,48.9],
"precipitation":[0.0,0.0],"precipitation_probability":[5,1],
"wind_speed_10m":[2.5,1.1],"wind_gusts_10m":[4.0,4.5],"cloud_cover":[99,99],
"weather_code":[3,3]}}"#;

/// One OpenWeatherMap slot.
const OWM: &str = r#"{"list":[{"dt":1795348800,
 "main":{"temp":50.0,"feels_like":48.0,"humidity":70.0},
 "weather":[{"id":800,"main":"Clear","description":"clear sky"}],
 "clouds":{"all":0.0},"wind":{"speed":3.0},"pop":0.0}]}"#;

fn with_open_meteo_at(base: &str) -> Clients {
    let mut clients = unreachable();
    clients.open_meteo =
        OpenMeteoClient::new(Arc::new(Outside::unpaced())).with_urls(base, "http://127.0.0.1:1");
    clients
}

fn with_owm_at(base: &str) -> Clients {
    let mut clients = unreachable();
    clients.owm = OpenWeatherMapClient::new().with_base_url(base);
    clients
}

async fn store_owm_key(pool: &PgPool, key: &str) {
    let held = Credential {
        address: None,
        secret: key.into(),
    };
    setup_queries::store_credential(pool, Service::OpenWeatherMap, &held)
        .await
        .unwrap();
}

// @spec SYNC-SCHED-019, SYNC-SCHED-003, SYNC-SCHED-020, SYNC-CONN-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_first_pass_fetches_the_forecast_for_the_lawn_before_current_data(pool: PgPool) {
    located(&pool).await;
    // The stub answers one request: the forecast, asked before the hourly analysis
    // that shares Open-Meteo's address.
    let (base, seen) = stub_http_seeing(200, OPEN_METEO, "").await;
    let (mut schedule, _, _) = Schedule::new(pool.clone(), with_open_meteo_at(&base));
    let slot = schedule.forecast_slot();
    schedule.pass(at(0)).await.unwrap();

    let request = seen.await.unwrap();
    let line = request.lines().next().unwrap();
    assert!(line.contains("latitude=39.86&longitude=-75.79"), "{line}");
    assert!(
        line.contains("apparent_temperature"),
        "the forecast: {line}"
    );
    let forecast = slot.forecast().expect("a forecast is held");
    assert_eq!(forecast.provider, ForecastProvider::OpenMeteo);
    assert_eq!(forecast.fetched_at, at(0));
    assert_eq!(
        status(&pool, &slot).await.unwrap(),
        ForecastStatus {
            provider: ForecastProvider::OpenMeteo,
            fetched_at: Some(at(0)),
            last_error: None,
        }
    );
}

// @spec SYNC-SCHED-019, WX-SRC-FC-010, WX-SRC-CFG-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn openweathermap_is_asked_with_the_stored_key_for_the_lawns_point(pool: PgPool) {
    located(&pool).await;
    store_owm_key(&pool, "stored-owm-key").await;
    setup_queries::set_forecast_provider(&pool, ForecastProvider::OpenWeatherMap)
        .await
        .unwrap();
    let (base, seen) = stub_http_seeing(200, OWM, "").await;
    let (mut schedule, _, _) = Schedule::new(pool.clone(), with_owm_at(&base));
    let slot = schedule.forecast_slot();
    schedule.pass(at(0)).await.unwrap();

    let line = seen.await.unwrap();
    let line = line.lines().next().unwrap();
    for part in ["lat=39.86", "lon=-75.79", "appid=stored-owm-key"] {
        assert!(line.contains(part), "{part} missing from {line}");
    }
    assert_eq!(
        slot.forecast().unwrap().provider,
        ForecastProvider::OpenWeatherMap
    );
}

// @spec SYNC-SCHED-023, WX-SRC-FC-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn openweathermap_with_no_key_is_not_asked_and_no_other_provider_is(pool: PgPool) {
    located(&pool).await;
    setup_queries::set_forecast_provider(&pool, ForecastProvider::OpenWeatherMap)
        .await
        .unwrap();
    // Both stubs would answer; neither may be asked for a forecast.
    let (owm_base, mut owm_seen) = stub_http_seeing(200, OWM, "").await;
    let (om_base, mut om_seen) = stub_http_seeing(200, OPEN_METEO, "").await;
    let mut clients = with_owm_at(&owm_base);
    clients.open_meteo = OpenMeteoClient::new(Arc::new(Outside::unpaced()))
        .with_urls(&om_base, "http://127.0.0.1:1");
    let (mut schedule, _, _) = Schedule::new(pool.clone(), clients);
    let slot = schedule.forecast_slot();
    schedule.pass(at(0)).await.unwrap();

    assert!(owm_seen.try_recv().is_err(), "OpenWeatherMap was asked");
    if let Ok(request) = om_seen.try_recv() {
        assert!(
            !request.contains("apparent_temperature"),
            "Open-Meteo was asked for a forecast: {request}"
        );
    }
    assert!(slot.forecast().is_none());
    assert_eq!(
        status(&pool, &slot).await.unwrap(),
        ForecastStatus {
            provider: ForecastProvider::OpenWeatherMap,
            fetched_at: None,
            last_error: Some(needs_key(ForecastProvider::OpenWeatherMap)),
        }
    );
}

// @spec SYNC-SCHED-022, SYNC-CONN-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_failed_forecast_fetch_is_its_status(pool: PgPool) {
    located(&pool).await;
    let (mut schedule, _, _) = Schedule::new(pool.clone(), unreachable());
    let slot = schedule.forecast_slot();
    schedule.pass(at(0)).await.unwrap();

    let reported = status(&pool, &slot).await.unwrap();
    assert_eq!(reported.provider, ForecastProvider::OpenMeteo);
    assert_eq!(reported.fetched_at, None);
    let said = reported.last_error.expect("the failure is reported");
    assert!(said.contains("forecast"), "{said}");
    assert!(!said.contains("127.0.0.1"), "no upstream detail: {said}");
}

// @spec SYNC-CONN-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_status_names_the_provider_chosen_now(pool: PgPool) {
    located(&pool).await;
    let (base, _) = stub_http_seeing(200, OPEN_METEO, "").await;
    let (mut schedule, _, _) = Schedule::new(pool.clone(), with_open_meteo_at(&base));
    let slot = schedule.forecast_slot();
    schedule.pass(at(0)).await.unwrap();
    setup_queries::set_forecast_provider(&pool, ForecastProvider::NationalWeatherService)
        .await
        .unwrap();

    // Before the schedule's next turn: nothing fetched from the new provider yet.
    assert_eq!(
        status(&pool, &slot).await.unwrap(),
        ForecastStatus {
            provider: ForecastProvider::NationalWeatherService,
            fetched_at: None,
            last_error: None,
        }
    );
}

// @spec SYNC-SCHED-009, SYNC-MERGE-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_refresh_pass_fetches_the_forecast_again_and_the_passes_are_counted(pool: PgPool) {
    located(&pool).await;
    store_owm_key(&pool, "stored-owm-key").await;
    setup_queries::set_forecast_provider(&pool, ForecastProvider::OpenWeatherMap)
        .await
        .unwrap();
    let (base, mut requests) =
        crate::test_support::stub_http_sequence(vec![(200, OWM), (200, OWM)]).await;
    let (mut schedule, waker, _) = Schedule::new(pool.clone(), with_owm_at(&base));
    assert_eq!((waker.passes(), waker.changes()), (0, 0));

    schedule.pass(at(0)).await.unwrap();
    assert_eq!(waker.passes(), 1);
    assert_eq!(waker.changes(), 1, "the plan was built and a forecast kept");
    assert!(
        requests.try_recv().is_ok(),
        "the first pass fetched the forecast"
    );

    schedule.pass(at(2)).await.unwrap();
    assert!(requests.try_recv().is_err(), "not again within the hour");
    assert_eq!(waker.passes(), 2);

    waker.refresh();
    schedule.pass(at(3)).await.unwrap();
    assert!(requests.try_recv().is_ok(), "a refresh fetches it again");
    assert_eq!(waker.passes(), 3);
    assert!(
        waker.changes() >= 2,
        "the forecast kept changed current data"
    );
}

// @spec SYNC-SCHED-009
#[tokio::test]
async fn waiting_for_a_pass_ends_when_one_finishes_or_the_wait_runs_out() {
    let pool = crate::logic::data_sync::test_env::unreachable_pool();
    let (_schedule, waker, _) = Schedule::new(pool, unreachable());
    let started = std::time::Instant::now();
    assert!(
        !waker
            .pass_after(0, std::time::Duration::from_millis(200))
            .await,
        "no pass ran"
    );
    assert!(started.elapsed() >= std::time::Duration::from_millis(200));
}
