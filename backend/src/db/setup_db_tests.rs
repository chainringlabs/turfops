//! The owner's source choices against PostgreSQL.

use super::setup_queries::*;
use super::weather_queries::{catalog, plan};
use crate::logic::weather_sync::planning;
use crate::models::forecast::ForecastProvider;
use crate::models::weather_record::*;
use crate::models::{GrassType, LawnLocation};
use sqlx::PgPool;

fn probe(input: Input) -> NewSensor {
    NewSensor {
        name: "Probe".into(),
        entity_id: Some("sensor.probe".into()),
        input,
        rain_kind: None,
    }
}

// @spec SETUP-MAP-003, SETUP-MAP-008, SETUP-MAP-009
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_sensor_has_one_input_and_an_input_one_sensor(pool: PgPool) {
    let first = map_sensor(&pool, &probe(Input::SoilTemp10)).await.unwrap();
    assert_eq!(first.way_in, WayIn::HomeAssistant);
    assert_eq!(first.source_key(), format!("sensor:{}", first.id));

    // The same entity cannot speak for a second input.
    assert!(map_sensor(&pool, &probe(Input::SoilTemp5)).await.is_err());
    // Nor a second sensor for the same input.
    let mut other = probe(Input::SoilTemp10);
    other.entity_id = Some("sensor.other".into());
    assert!(map_sensor(&pool, &other).await.is_err());

    let posted = NewSensor {
        name: "Rain gauge".into(),
        entity_id: None,
        input: Input::Rain,
        rain_kind: Some(RainKind::Counter),
    };
    let gauge = map_sensor(&pool, &posted).await.unwrap();
    assert_eq!(
        (gauge.way_in, gauge.rain_kind),
        (WayIn::Posted, Some(RainKind::Counter))
    );
    assert_eq!(sensors(&pool).await.unwrap().len(), 2);

    // Unmapping frees the input and the entity; the row stays.
    assert!(unmap_sensor(&pool, first.id).await.unwrap());
    assert!(
        !unmap_sensor(&pool, first.id).await.unwrap(),
        "already unmapped"
    );
    assert_eq!(sensors(&pool).await.unwrap(), vec![gauge]);
    map_sensor(&pool, &probe(Input::SoilTemp5)).await.unwrap();
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM sensor")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kept, 3);
}

// @spec SETUP-MAP-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_rain_sensor_says_what_its_number_means_and_no_other_sensor_does(pool: PgPool) {
    assert!(
        map_sensor(&pool, &probe(Input::Rain)).await.is_err(),
        "rain with no kind"
    );
    let mut air = probe(Input::AirTemp);
    air.rain_kind = Some(RainKind::Amount);
    assert!(
        map_sensor(&pool, &air).await.is_err(),
        "a kind on a thermometer"
    );
}

fn station(id: &str, miles_north: f64) -> Station {
    Station {
        network: Network::Scan,
        station_id: id.into(),
        name: format!("Station {id}"),
        state: "PA".into(),
        latitude: 39.86 + miles_north / 69.09,
        longitude: -75.79,
        elevation_ft: Some(400.0),
        inputs: vec![Input::SoilTemp10],
        first_year: Some(2006),
        last_year: None,
        locator: None,
        utc_offset_hours: None,
    }
}

// @spec SETUP-API-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_pin_is_one_station_per_input_and_goes_with_its_station(pool: PgPool) {
    let listed = [station("near", 10.0), station("far", 120.0)];
    catalog::replace_network(&pool, Network::Scan, &listed)
        .await
        .unwrap();
    let held = catalog::stations(&pool).await.unwrap();
    let (far, near) = (held[0].0, held[1].0);
    pin(&pool, Input::SoilTemp10, near).await.unwrap();
    pin(&pool, Input::SoilTemp10, far).await.unwrap();
    assert_eq!(pins(&pool).await.unwrap(), vec![(Input::SoilTemp10, far)]);
    assert!(unpin(&pool, Input::SoilTemp10).await.unwrap());
    assert!(!unpin(&pool, Input::SoilTemp10).await.unwrap());

    pin(&pool, Input::SoilTemp10, far).await.unwrap();
    catalog::replace_network(&pool, Network::Scan, &listed[..1])
        .await
        .unwrap();
    assert!(
        pins(&pool).await.unwrap().is_empty(),
        "the network dropped the station"
    );
}

// @spec WX-SRC-FC-001, SETUP-PLAN-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_forecast_provider_is_open_meteo_until_the_owner_chooses(pool: PgPool) {
    assert_eq!(
        forecast_provider(&pool).await.unwrap(),
        ForecastProvider::OpenMeteo
    );
    set_forecast_provider(&pool, ForecastProvider::NationalWeatherService)
        .await
        .unwrap();
    set_forecast_provider(&pool, ForecastProvider::OpenWeatherMap)
        .await
        .unwrap();
    assert_eq!(
        forecast_provider(&pool).await.unwrap(),
        ForecastProvider::OpenWeatherMap
    );
}

// @spec SETUP-CRED-001, SETUP-CRED-002, SETUP-CRED-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_forgotten_connection_is_gone_and_is_remembered_as_having_been_set(pool: PgPool) {
    let service = Service::HomeAssistant;
    assert_eq!(credential(&pool, service).await.unwrap(), None);
    assert!(!credential_ever_stored(&pool, service).await.unwrap());

    let held = Credential {
        address: Some("http://ha.example:8123".into()),
        secret: "token-0123456789".into(),
    };
    store_credential(&pool, service, &held).await.unwrap();
    assert_eq!(
        credential(&pool, service).await.unwrap(),
        Some(held.clone())
    );
    assert!(
        !format!("{held:?}").contains("token-0123456789"),
        "{held:?}"
    );
    assert_eq!(
        credential(&pool, Service::OpenWeatherMap).await.unwrap(),
        None
    );

    forget_credential(&pool, service).await.unwrap();
    assert_eq!(credential(&pool, service).await.unwrap(), None);
    assert!(credential_ever_stored(&pool, service).await.unwrap());
    let left: Option<String> =
        sqlx::query_scalar("SELECT secret FROM service_credential WHERE service = 'HomeAssistant'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(left, None, "the secret itself is deleted");
}

// @spec WX-REC-PLAN-002, WX-REC-PLAN-003, WX-REC-PLAN-008, SETUP-MAP-009
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_plan_follows_the_owners_sensors_and_pins(pool: PgPool) {
    let mut profile = crate::logic::rules::test_fixtures::profile(GrassType::TallFescue);
    profile.id = None;
    profile.location = Some(LawnLocation::at(39.86, -75.79, None).unwrap());
    crate::db::queries::create_lawn_profile(&pool, &profile)
        .await
        .unwrap();
    catalog::replace_network(
        &pool,
        Network::Scan,
        &[station("near", 10.0), station("far", 120.0)],
    )
    .await
    .unwrap();
    let soil = |placed: Vec<plan::Placed>| -> Vec<String> {
        placed
            .into_iter()
            .filter(|p| p.input == Input::SoilTemp10)
            .map(|p| p.source.key)
            .collect()
    };
    assert!(planning::rebuild(&pool).await.unwrap());
    assert!(!planning::rebuild(&pool).await.unwrap(), "nothing changed");
    let estimate = "estimate:39.8600,-75.7900".to_string();
    assert_eq!(
        soil(plan::read(&pool).await.unwrap()),
        vec!["station:Scan:near".to_string(), estimate.clone()]
    );

    let sensor = map_sensor(&pool, &probe(Input::SoilTemp10)).await.unwrap();
    let far = catalog::stations(&pool).await.unwrap()[0].0;
    pin(&pool, Input::SoilTemp10, far).await.unwrap();
    assert!(planning::rebuild(&pool).await.unwrap());
    assert_eq!(
        soil(plan::read(&pool).await.unwrap()),
        vec![
            sensor.source_key(),
            "station:Scan:far".to_string(),
            estimate.clone()
        ]
    );

    unmap_sensor(&pool, sensor.id).await.unwrap();
    unpin(&pool, Input::SoilTemp10).await.unwrap();
    assert!(planning::rebuild(&pool).await.unwrap());
    assert_eq!(
        soil(plan::read(&pool).await.unwrap()),
        vec!["station:Scan:near".to_string(), estimate]
    );
}
