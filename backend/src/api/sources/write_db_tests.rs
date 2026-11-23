//! The Sources read and the writes that change it, against PostgreSQL.

use super::read::SourcesView;
use super::write::{
    delete_pin, put_forecast_provider, put_pin, ForecastProviderRequest, PinRequest,
};
use crate::api::applications::db_support::{lawn, reply, state};
use crate::api::setup::{apply, ApplyRequest};
use crate::db::setup_queries::{self, Service};
use crate::db::weather_queries::{catalog, lawn as lawn_queries, plan};
use crate::logic::weather_sync::planning;
use crate::models::forecast::ForecastProvider;
use crate::models::weather_record::*;
use crate::models::LawnLocation;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::json;
use sqlx::PgPool;

type Answer = Result<SourcesView, (StatusCode, String)>;

pub(super) fn station(network: Network, id: &str, miles_north: f64, inputs: &[Input]) -> Station {
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

pub(super) const SOIL: [Input; 2] = [Input::SoilTemp5, Input::SoilTemp10];

/// A lawn near Avondale with its plan built, among a USCRN station at 20 miles and SCAN
/// stations at 60, 140 and 200 miles (soil) and 5 miles (air only).
pub(super) async fn located(pool: &PgPool) -> AppState {
    let uscrn = [station(Network::Uscrn, "near", 20.0, &SOIL)];
    let scan = [
        station(Network::Scan, "mid", 60.0, &SOIL),
        station(Network::Scan, "far", 140.0, &SOIL),
        station(Network::Scan, "beyond", 200.0, &SOIL),
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
    lawn(pool).await;
    let mut profile = crate::db::queries::get_default_lawn_profile(pool)
        .await
        .unwrap()
        .unwrap();
    profile.location = Some(LawnLocation::at(39.86, -75.79, Some("Avondale".into())).unwrap());
    crate::db::queries::update_lawn_profile(pool, &profile)
        .await
        .unwrap();
    planning::rebuild(pool).await.unwrap();
    state(pool).await
}

pub(super) async fn station_ref(pool: &PgPool, id: &str) -> i64 {
    let held = catalog::stations(pool).await.unwrap();
    held.into_iter()
        .find(|(_, s)| s.station_id == id)
        .unwrap()
        .0
}

pub(super) fn first_source<'a>(view: &'a SourcesView, input: &str) -> &'a str {
    let found = view.inputs.iter().find(|i| i.plan.input == input).unwrap();
    found.plan.sources[0].name.as_str()
}

async fn pin_to(state: &AppState, input: &str, station_ref: i64) -> Answer {
    let request = Json(PinRequest { station_ref });
    match put_pin(State(state.clone()), Path(input.to_string()), request).await {
        Ok(Json(view)) => Ok(view),
        Err(err) => Err(reply(err).await),
    }
}

// @spec SETUP-API-005, SETUP-API-010, SETUP-PLAN-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn pinning_a_station_changes_the_plan_at_once_and_unpinning_restores_it(pool: PgPool) {
    let state = located(&pool).await;
    let far = station_ref(&pool, "far").await;
    let view = pin_to(&state, "soil_temp_10", far).await.unwrap();
    assert_eq!(
        first_source(&view, "soil_temp_10"),
        "Station far, PA (SCAN)"
    );
    let pinned = view
        .inputs
        .iter()
        .find(|i| i.plan.input == "soil_temp_10")
        .unwrap();
    assert_eq!(pinned.pinned_station_ref, Some(far));
    assert!(pinned.plan.sources[0].pinned);
    let stored = plan::read(&pool).await.unwrap();
    assert!(stored
        .iter()
        .any(|p| p.pinned && p.source.key == "station:Scan:far"));

    let view = delete_pin(State(state), Path("soil_temp_10".to_string()))
        .await
        .unwrap()
        .0;
    assert_eq!(
        first_source(&view, "soil_temp_10"),
        "Station near, PA (USCRN)"
    );
}

// @spec SETUP-API-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_pin_that_cannot_stand_says_why_and_changes_nothing(pool: PgPool) {
    let state = located(&pool).await;
    let air = station_ref(&pool, "air").await;
    let beyond = station_ref(&pool, "beyond").await;
    let refused = |result: Answer| {
        let (status, message) = result.expect_err("refused");
        assert_eq!(status, StatusCode::BAD_REQUEST, "{message}");
        message
    };
    assert_eq!(
        refused(pin_to(&state, "soil_temp_10", air).await),
        "Station air does not report soil temperature, 10 cm."
    );
    assert!(refused(pin_to(&state, "soil_temp_10", beyond).await).contains("within 150 miles"));
    assert!(refused(pin_to(&state, "soil_temp_10", 9999).await).contains("not in the station"));
    assert!(refused(pin_to(&state, "soil", air).await).contains("is not an input"));
    assert!(setup_queries::pins(&pool).await.unwrap().is_empty());
}

async fn provider(state: &AppState, body: serde_json::Value) -> Answer {
    let request: ForecastProviderRequest = serde_json::from_value(body).unwrap();
    match put_forecast_provider(State(state.clone()), Json(request)).await {
        Ok(Json(view)) => Ok(view),
        Err(err) => Err(reply(err).await),
    }
}

// @spec SETUP-API-006, SETUP-PLAN-004, SETUP-CRED-001, WX-SRC-CFG-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn openweathermap_needs_a_key_and_the_key_is_stored_and_never_returned(pool: PgPool) {
    let state = located(&pool).await;
    let (status, message) = provider(&state, json!({"provider": "OpenWeatherMap"}))
        .await
        .err()
        .unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        message,
        "OpenWeatherMap needs a key. Enter one, or choose another provider."
    );
    assert_eq!(
        setup_queries::forecast_provider(&pool).await.unwrap(),
        ForecastProvider::OpenMeteo
    );

    let secret = "owm-secret-key-0123456789";
    let chosen = json!({"provider": "OpenWeatherMap", "key": secret});
    let view = provider(&state, chosen).await.unwrap();
    assert_eq!(view.forecast_provider, ForecastProvider::OpenWeatherMap);
    assert!(view.connections.openweathermap_key);
    assert!(!serde_json::to_string(&view).unwrap().contains(secret));
    let stored = setup_queries::credential(&pool, Service::OpenWeatherMap)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.secret, secret);

    // Another provider keeps the key; choosing OpenWeatherMap again needs none given.
    let view = provider(&state, json!({"provider": "NationalWeatherService"}))
        .await
        .unwrap();
    assert_eq!(
        view.forecast_provider,
        ForecastProvider::NationalWeatherService
    );
    let again = json!({"provider": "OpenWeatherMap", "key": "  "});
    assert!(provider(&state, again).await.is_ok());
}

async fn applying(state: &AppState, body: serde_json::Value) -> Answer {
    let request: ApplyRequest = serde_json::from_value(body).unwrap();
    match apply(State(state.clone()), Json(request)).await {
        Ok(Json(view)) => Ok(view),
        Err(err) => Err(reply(err).await),
    }
}

// @spec SETUP-API-012, SETUP-PLAN-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn setup_saves_a_location_pins_and_a_provider_together(pool: PgPool) {
    let state = located(&pool).await;
    lawn_queries::set_elevation_ft(&pool, 446.0).await.unwrap();
    let mid = station_ref(&pool, "mid").await;
    // Half a degree north: the lawn moves about 35 miles.
    let body = json!({
        "location": {"latitude": 40.36, "longitude": -75.79, "place_name": "Quakertown"},
        "pins": [{"input": "soil_temp_5", "station_ref": mid}],
        "forecast_provider": {"provider": "NationalWeatherService"}
    });
    let view = applying(&state, body).await.unwrap();
    let location = view.location.as_ref().unwrap();
    assert_eq!(
        (location.latitude, location.place_name.as_deref()),
        (40.36, Some("Quakertown"))
    );
    assert_eq!(
        view.forecast_provider,
        ForecastProvider::NationalWeatherService
    );
    assert_eq!(first_source(&view, "soil_temp_5"), "Station mid, PA (SCAN)");
    assert_eq!(
        first_source(&view, "soil_temp_10"),
        "Station near, PA (USCRN)"
    );
    assert_eq!(first_source(&view, "rain"), "PRISM daily grid");
    let stored = plan::read(&pool).await.unwrap();
    assert!(
        stored
            .iter()
            .any(|p| p.source.key == "grid:prism:40.3600,-75.7900"),
        "the new point"
    );
    assert_eq!(
        lawn_queries::elevation_ft(&pool).await.unwrap(),
        None,
        "the old point's elevation"
    );
}

// @spec SETUP-API-012
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_setup_save_with_one_bad_part_saves_none_of_it(pool: PgPool) {
    let state = located(&pool).await;
    let air = station_ref(&pool, "air").await;
    let body = json!({
        "location": {"latitude": 40.36, "longitude": -75.79},
        "pins": [{"input": "soil_temp_10", "station_ref": air}],
        "forecast_provider": {"provider": "NationalWeatherService"}
    });
    let (status, message) = applying(&state, body).await.err().unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("does not report"), "{message}");
    let profile = crate::db::queries::get_default_lawn_profile(&pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(profile.location.unwrap().latitude, 39.86);
    assert_eq!(
        setup_queries::forecast_provider(&pool).await.unwrap(),
        ForecastProvider::OpenMeteo
    );

    let outside = json!({"location": {"latitude": 21.3, "longitude": -157.86}});
    let (status, message) = applying(&state, outside).await.err().unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(message.contains("no weather data for that location yet"));
}

// @spec SETUP-API-012
#[sqlx::test(migrations = "src/db/migrations")]
async fn setup_gives_a_lawn_its_first_location(pool: PgPool) {
    let fresh = state(&pool).await;
    let body = json!({"location": {"latitude": 39.86, "longitude": -75.79}});
    let (status, _) = applying(&fresh, body.clone()).await.err().unwrap();
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "no lawn profile to save it to"
    );
    lawn(&pool).await;
    let view = applying(&fresh, body).await.unwrap();
    assert_eq!(view.location.unwrap().timezone, "America/New_York");
    assert!(
        view.sources.iter().any(|s| s.kind == SourceKind::Estimate),
        "the plan was built"
    );
}
