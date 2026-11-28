//! `/api/v1/sources/preview` and `/sources/stations` against PostgreSQL.

use super::view::Standing;
use super::{preview, stations_for, PreviewQuery, StationsQuery};
use crate::api::applications::db_support::{reply, state};
use crate::db::weather_queries::{catalog, plan};
use crate::models::weather_record::*;
use crate::models::LawnLocation;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use sqlx::PgPool;

fn station(network: Network, id: &str, miles_north: f64, inputs: &[Input]) -> Station {
    Station {
        network,
        station_id: id.into(),
        name: format!("Station {id}"),
        state: "PA".into(),
        latitude: 39.86 + miles_north / 69.09,
        longitude: -75.79,
        elevation_ft: Some(400.0),
        inputs: inputs.to_vec(),
        first_year: Some(2006),
        last_year: None,
        locator: None,
        utc_offset_hours: None,
    }
}

const SOIL: [Input; 2] = [Input::SoilTemp5, Input::SoilTemp10];

/// A USCRN station 20 miles off and SCAN stations at 60 and 140 miles, all with soil.
pub(super) async fn stocked(pool: &PgPool) -> AppState {
    let uscrn = [station(Network::Uscrn, "near", 20.0, &SOIL)];
    let scan = [
        station(Network::Scan, "mid", 60.0, &SOIL),
        station(Network::Scan, "far", 140.0, &SOIL),
        station(
            Network::Scan,
            "air",
            5.0,
            &[Input::AirTemp, Input::Humidity],
        ),
    ];
    catalog::replace_network(pool, Network::Uscrn, &uscrn)
        .await
        .unwrap();
    catalog::replace_network(pool, Network::Scan, &scan)
        .await
        .unwrap();
    state(pool).await
}

pub(super) fn at(lat: f64, lon: f64, pins: Option<&str>) -> Query<PreviewQuery> {
    Query(PreviewQuery {
        lat,
        lon,
        pins: pins.map(str::to_string),
    })
}

// @spec SETUP-API-002, SETUP-LOC-003, SETUP-PLAN-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_preview_shows_what_the_point_would_get_and_saves_nothing(pool: PgPool) {
    let state = stocked(&pool).await;
    let got = preview(State(state), at(39.86004, -75.79, None))
        .await
        .unwrap()
        .0;
    assert_eq!(
        (got.latitude, got.longitude),
        (39.86, -75.79),
        "as it will be stored"
    );
    assert_eq!(got.timezone, "America/New_York");
    let nearest = got.nearest_soil_station.unwrap();
    assert_eq!(
        (nearest.name.as_str(), nearest.network),
        ("Station near", "USCRN")
    );
    assert!((nearest.distance_miles - 20.0).abs() < 0.05);

    let input = |name: &str| got.inputs.iter().find(|i| i.input == name).unwrap();
    let soil = input("soil_temp_10");
    assert_eq!(soil.label, "Soil temperature, 10 cm");
    assert_eq!(soil.standing, Standing::Measured);
    let names: Vec<_> = soil.sources.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Station near, PA (USCRN)", "Soil temperature estimate"]
    );
    assert_eq!(
        input("air_temp").standing,
        Standing::Measured,
        "the SCAN station 5 miles off"
    );
    assert_eq!(input("rain").standing, Standing::Estimated);
    assert_eq!(input("soil_moisture_10").standing, Standing::NoSource);
    assert!(input("soil_moisture_10").sources.is_empty());
    assert_eq!(got.inputs.len(), 14, "every input is listed");

    assert!(
        plan::read(&pool).await.unwrap().is_empty(),
        "nothing was saved"
    );
}

// @spec SETUP-PLAN-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_preview_with_a_pin_shows_the_plan_with_the_change(pool: PgPool) {
    let state = stocked(&pool).await;
    let far = catalog::stations(&pool)
        .await
        .unwrap()
        .into_iter()
        .find(|(_, s)| s.station_id == "far")
        .unwrap()
        .0;
    let pins = format!("soil_temp_10:{far}");
    let got = preview(State(state.clone()), at(39.86, -75.79, Some(&pins)))
        .await
        .unwrap()
        .0;
    let soil = got
        .inputs
        .iter()
        .find(|i| i.input == "soil_temp_10")
        .unwrap();
    assert_eq!(soil.sources[0].name, "Station far, PA (SCAN)");
    assert!(soil.sources[0].pinned);
    let five = got
        .inputs
        .iter()
        .find(|i| i.input == "soil_temp_5")
        .unwrap();
    assert_eq!(
        five.sources[0].name, "Station near, PA (USCRN)",
        "other inputs keep their defaults"
    );

    let err = preview(State(state), at(39.86, -75.79, Some("soil_temp_10")))
        .await
        .unwrap_err();
    assert_eq!(reply(err).await.0, StatusCode::BAD_REQUEST);
}

// @spec SETUP-LOC-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_point_outside_the_contiguous_united_states_is_refused(pool: PgPool) {
    let state = stocked(&pool).await;
    let err = preview(State(state), at(21.3, -157.86, None))
        .await
        .unwrap_err();
    let (status, message) = reply(err).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        message.contains("no weather data for that location yet"),
        "{message}"
    );
}

// @spec SETUP-FAIL-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_no_station_list_and_no_way_to_load_one_the_preview_is_estimates(pool: PgPool) {
    // No clients in this state: the list cannot be loaded, and the plan has no stations.
    let got = preview(State(state(&pool).await), at(39.86, -75.79, None))
        .await
        .unwrap()
        .0;
    assert!(got.nearest_soil_station.is_none());
    let soil = got
        .inputs
        .iter()
        .find(|i| i.input == "soil_temp_10")
        .unwrap();
    assert_eq!(soil.standing, Standing::Estimated);
}

// @spec SETUP-FAIL-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_station_list_that_cannot_be_loaded_is_an_outage_of_the_station_network(pool: PgPool) {
    let fetcher = crate::api::setup::db_tests::searching(&pool, "http://127.0.0.1:1");
    let state = state(&pool).await.fetching(fetcher);
    let err = preview(State(state), at(39.86, -75.79, None))
        .await
        .unwrap_err();
    let (status, message) = reply(err).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(message.contains("weather station network"), "{message}");
}

// @spec SETUP-FAIL-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_network_whose_list_is_missing_is_loaded_even_when_the_other_is_held(pool: PgPool) {
    let uscrn = [station(Network::Uscrn, "near", 20.0, &SOIL)];
    catalog::replace_network(&pool, Network::Uscrn, &uscrn)
        .await
        .unwrap();
    // The SCAN list is missing and cannot be loaded: the preview is the network's outage,
    // not a plan that silently lacks every SCAN station.
    let fetcher = crate::api::setup::db_tests::searching(&pool, "http://127.0.0.1:1");
    let state = state(&pool).await.fetching(fetcher);
    let err = preview(State(state.clone()), at(39.86, -75.79, None))
        .await
        .unwrap_err();
    let (status, message) = reply(err).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(message.contains("weather station network"), "{message}");
    let err = stations_for(
        State(state),
        stations_query("soil_temp_10", Some((39.86, -75.79))),
    )
    .await
    .unwrap_err();
    assert_eq!(reply(err).await.0, StatusCode::SERVICE_UNAVAILABLE);
}

pub(super) fn stations_query(input: &str, point: Option<(f64, f64)>) -> Query<StationsQuery> {
    Query(StationsQuery {
        input: input.into(),
        lat: point.map(|p| p.0),
        lon: point.map(|p| p.1),
    })
}

// @spec SETUP-API-004, SETUP-PLAN-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_stations_for_an_input_are_those_within_150_miles_that_may_report_it(pool: PgPool) {
    let state = stocked(&pool).await;
    let point = Some((39.86, -75.79));
    let got = stations_for(State(state.clone()), stations_query("soil_temp_10", point))
        .await
        .unwrap()
        .0;
    let names: Vec<_> = got.iter().map(|s| (s.name.as_str(), s.network)).collect();
    assert_eq!(
        names,
        vec![
            ("Station near", "USCRN"),
            ("Station mid", "SCAN"),
            ("Station far", "SCAN")
        ]
    );
    assert_eq!((got[0].first_year, got[0].last_year), (Some(2006), None));
    assert_eq!(
        got[0].elevation_difference_ft, None,
        "the point's elevation is not known yet"
    );

    let err = stations_for(State(state.clone()), stations_query("soil", point))
        .await
        .unwrap_err();
    assert_eq!(reply(err).await.0, StatusCode::BAD_REQUEST);

    // With no point given it is the lawn's, and a lawn with no location has none.
    let err = stations_for(State(state.clone()), stations_query("soil_temp_10", None))
        .await
        .unwrap_err();
    assert!(reply(err)
        .await
        .1
        .contains("doesn't know where your lawn is"));
    let id = crate::api::applications::db_support::lawn(&pool).await;
    let mut profile = crate::db::queries::get_default_lawn_profile(&pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(profile.id, Some(id));
    profile.location = Some(LawnLocation::at(39.86, -75.79, None).unwrap());
    crate::db::queries::update_lawn_profile(&pool, &profile)
        .await
        .unwrap();
    let got = stations_for(State(state), stations_query("air_temp", None))
        .await
        .unwrap()
        .0;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].name, "Station air");
}

// @spec SETUP-API-002, SETUP-PLAN-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_preview_counts_the_sensors_already_mapped(pool: PgPool) {
    let state = stocked(&pool).await;
    let patio = crate::db::setup_queries::NewSensor {
        name: "Patio temperature".into(),
        entity_id: Some("sensor.patio".into()),
        input: Input::AirTemp,
        rain_kind: None,
    };
    crate::db::setup_queries::map_sensor(&pool, &patio)
        .await
        .unwrap();
    let got = preview(State(state), at(39.86, -75.79, None))
        .await
        .unwrap()
        .0;
    let air = got.inputs.iter().find(|i| i.input == "air_temp").unwrap();
    assert_eq!(air.sources[0].name, "Patio temperature");
    assert_eq!(air.sources[0].kind, SourceKind::Sensor);
    assert_eq!(air.standing, Standing::Measured);
    assert!(
        plan::read(&pool).await.unwrap().is_empty(),
        "still nothing saved"
    );
}
