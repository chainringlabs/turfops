//! The elevation guard during a preview, with Open-Meteo stubbed.

use super::db_tests::{at, stations_query, stocked};
use super::{preview, stations_for};
use crate::state::AppState;
use axum::extract::State;
use sqlx::PgPool;

const POINT_AT_446_FT: &str = r#"{"elevation":136.0,
"hourly_units":{"time":"unixtime","temperature_2m":"°F","relative_humidity_2m":"%","precipitation":"mm"},
"hourly":{"time":[],"temperature_2m":[],"relative_humidity_2m":[],"precipitation":[]}}"#;

/// A state whose weather clients reach only an Open-Meteo that gives these answers.
async fn asking_open_meteo(pool: &PgPool, answers: Vec<(u16, &'static str)>) -> AppState {
    use crate::datasources::grid::GridClient;
    use crate::datasources::open_meteo::OpenMeteoClient;
    use crate::datasources::outside::Outside;
    use crate::datasources::scan::ScanClient;
    use crate::datasources::uscrn::UscrnClient;
    use crate::logic::weather_sync::fetcher::{Clients, Fetcher};
    use std::sync::Arc;
    let (base, _requests) = crate::test_support::stub_http_sequence(answers).await;
    let outside = Arc::new(Outside::unpaced());
    let nowhere = "http://127.0.0.1:1";
    let clients = Clients {
        uscrn: UscrnClient::new(outside.clone()).with_base(nowhere),
        scan: ScanClient::new(outside.clone()).with_base(nowhere),
        grid: GridClient::new(outside.clone()).with_url(nowhere),
        open_meteo: OpenMeteoClient::new(outside.clone()).with_urls(&base, nowhere),
        nws: crate::datasources::nws::NwsClient::new(outside).with_base_url(nowhere),
        owm: crate::datasources::OpenWeatherMapClient::at(nowhere),
    };
    stocked(pool)
        .await
        .fetching(Fetcher::new(pool.clone(), clients))
}

// @spec SETUP-API-002, WX-REC-PLAN-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_preview_applies_the_elevation_guard_with_the_points_own_elevation(pool: PgPool) {
    let state =
        asking_open_meteo(&pool, vec![(200, POINT_AT_446_FT), (200, POINT_AT_446_FT)]).await;
    // The USCRN station 20 miles off moves to a ridge 1,600 feet above the point.
    sqlx::query("UPDATE weather_station SET elevation_ft = 2050 WHERE station_id = 'near'")
        .execute(&pool)
        .await
        .unwrap();
    let got = preview(State(state.clone()), at(39.86, -75.79, None))
        .await
        .unwrap()
        .0;
    assert!((got.elevation_ft.unwrap() - 446.19).abs() < 0.01);
    let soil = got
        .inputs
        .iter()
        .find(|i| i.input == "soil_temp_10")
        .unwrap();
    // The ridge station is passed over, and the next soil station is beyond 50 miles.
    assert_eq!(soil.sources[0].name, "Soil temperature estimate");

    // The stations offered for pinning say how far above or below the point they are.
    let listed = stations_for(
        State(state),
        stations_query("soil_temp_10", Some((39.86, -75.79))),
    )
    .await
    .unwrap()
    .0;
    let ridge = listed.iter().find(|s| s.name == "Station near").unwrap();
    assert!((ridge.elevation_difference_ft.unwrap() - 1603.81).abs() < 0.01);
}

// @spec SETUP-API-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_preview_goes_on_without_the_guard_when_the_elevation_cannot_be_had(pool: PgPool) {
    let state = asking_open_meteo(&pool, vec![(500, "upstream exploded")]).await;
    sqlx::query("UPDATE weather_station SET elevation_ft = 2050 WHERE station_id = 'near'")
        .execute(&pool)
        .await
        .unwrap();
    let got = preview(State(state), at(39.86, -75.79, None))
        .await
        .unwrap()
        .0;
    assert_eq!(got.elevation_ft, None);
    let soil = got
        .inputs
        .iter()
        .find(|i| i.input == "soil_temp_10")
        .unwrap();
    assert_eq!(soil.sources[0].name, "Station near, PA (USCRN)");
}
