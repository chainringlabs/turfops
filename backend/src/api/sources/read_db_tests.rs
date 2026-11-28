//! `GET /api/v1/sources` against PostgreSQL.

use super::read::get_sources;
use super::view::Standing;
use super::write_db_tests::{first_source, located};
use crate::api::applications::db_support::state;
use crate::db::weather_queries::{fetch_state, rows, sources};
use crate::models::forecast::ForecastProvider;
use crate::models::weather_record::*;
use axum::extract::State;
use chrono::{DurationRound, Utc};
use sqlx::PgPool;

// @spec SETUP-API-003, SETUP-PAGE-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_sources_read_gives_inputs_sources_provider_connections_and_location(pool: PgPool) {
    let state = located(&pool).await;
    // The analysis reported the last three hours; PRISM's last fetch failed.
    let all = sources::all(&pool).await.unwrap();
    let analysis = all.iter().find(|s| s.kind == SourceKind::Analysis).unwrap();
    let this_hour = Utc::now()
        .duration_trunc(chrono::Duration::hours(1))
        .unwrap();
    let hours: Vec<HourlyRow> = (1..=3)
        .map(|back| HourlyRow {
            observed_at: this_hour - chrono::Duration::hours(back),
            air_temp_f: Some(60.0 + back as f64),
            ..Default::default()
        })
        .collect();
    let mut conn = pool.acquire().await.unwrap();
    rows::write_hourly(&mut conn, analysis.id, &hours)
        .await
        .unwrap();
    drop(conn);
    let prism = all
        .iter()
        .find(|s| s.key.starts_with("grid:prism"))
        .unwrap();
    let jan1 = chrono::NaiveDate::from_ymd_opt(2011, 1, 1).unwrap();
    fetch_state::want(&pool, prism.id, "grid_days", jan1)
        .await
        .unwrap();
    let said = "The daily grid can't be reached.";
    fetch_state::failed(&pool, prism.id, "grid_days", said, Utc::now())
        .await
        .unwrap();

    let view = get_sources(State(state)).await.unwrap().0;
    let location = view.location.as_ref().unwrap();
    assert_eq!(
        (location.place_name.as_deref(), location.timezone.as_str()),
        (Some("Avondale"), "America/New_York")
    );
    assert_eq!(view.inputs.len(), 14);
    let input = |name: &str| view.inputs.iter().find(|i| i.plan.input == name).unwrap();
    assert_eq!(input("soil_temp_10").plan.standing, Standing::Measured);
    assert_eq!(
        first_source(&view, "soil_temp_10"),
        "Station near, PA (USCRN)"
    );
    assert!(
        input("soil_temp_10").in_use.is_none(),
        "nothing has been fetched for it"
    );
    assert_eq!(input("soil_moisture_10").plan.standing, Standing::NoSource);

    // The SCAN station 5 miles off is first for air but holds nothing; the analysis speaks.
    let air = input("air_temp").in_use.as_ref().unwrap();
    assert_eq!(air.value, 61.0, "the newest hour");
    assert!(air.basis.approximate);

    let failing = &view.sources[0];
    assert_eq!(
        failing.name, "PRISM daily grid",
        "a failing source comes first"
    );
    assert_eq!(failing.last_error.as_deref(), Some(said));
    assert!(failing.attribution.contains("PRISM Climate Group"));
    assert!(failing.inputs.contains(&"daily_air") && failing.inputs.contains(&"rain"));
    let uscrn = view
        .sources
        .iter()
        .find(|s| s.name.contains("USCRN"))
        .unwrap();
    assert!(uscrn.attribution.contains("Climate Reference Network"));
    assert_eq!(view.forecast_provider, ForecastProvider::OpenMeteo);
    assert!(!view.connections.home_assistant && !view.connections.openweathermap_key);
}

// @spec SETUP-API-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_lawn_with_no_location_has_no_sources_and_every_input_without_one(pool: PgPool) {
    let view = get_sources(State(state(&pool).await)).await.unwrap().0;
    assert!(view.location.is_none() && view.sources.is_empty());
    assert!(view
        .inputs
        .iter()
        .all(|i| i.plan.standing == Standing::NoSource));
}

// @spec SETUP-API-015, SYNC-CONN-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_sources_read_gives_the_forecast_status(pool: PgPool) {
    use crate::logic::weather_sync::forecast::{Choice, ForecastKeeper, ForecastSlot, Outcome};
    let state = located(&pool).await;
    let slot = ForecastSlot::new();
    let mut keeper = ForecastKeeper::new(slot.clone());
    let open_meteo = Choice {
        provider: ForecastProvider::OpenMeteo,
        key: None,
    };
    let failed_at = Utc::now();
    keeper.keep(
        &open_meteo,
        Outcome::Failed("No forecast right now.".into()),
        failed_at,
    );
    state.sync_service.write().await.use_forecast(slot);

    let axum::Json(view) = get_sources(State(state)).await.unwrap();
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(
        json["forecast"],
        serde_json::json!({
            "provider": "OpenMeteo",
            "fetched_at": null,
            "last_error": "No forecast right now."
        }),
        "{json}"
    );
}

// @spec SETUP-FAIL-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_mapped_entity_home_assistant_no_longer_has_is_flagged_on_its_source(pool: PgPool) {
    use crate::db::setup_queries::{self, NewSensor};
    use crate::logic::weather_sync::sensor_reading::Problem;
    use crate::logic::weather_sync::sensor_store::PRODUCT;
    let state = located(&pool).await;
    let map = |name: &str, entity: &str, input: Input| NewSensor {
        name: name.into(),
        entity_id: Some(entity.into()),
        input,
        rain_kind: None,
    };
    let gone = setup_queries::map_sensor(&pool, &map("Patio", "sensor.patio", Input::AirTemp))
        .await
        .unwrap();
    let offline = setup_queries::map_sensor(&pool, &map("Porch", "sensor.porch", Input::Humidity))
        .await
        .unwrap();
    crate::logic::weather_sync::planning::rebuild(&pool)
        .await
        .unwrap();
    let all = sources::all(&pool).await.unwrap();
    let source_of = |sensor: &setup_queries::Sensor| {
        all.iter()
            .find(|s| s.key == sensor.source_key())
            .unwrap()
            .id
    };
    let today = Utc::now().date_naive();
    for (sensor, problem) in [(&gone, Problem::NotFound), (&offline, Problem::NotANumber)] {
        let id = source_of(sensor);
        fetch_state::want(&pool, id, PRODUCT, today).await.unwrap();
        let entity = sensor.entity_id.as_deref().unwrap();
        let said = problem.sentence(sensor.input, entity);
        fetch_state::failed(&pool, id, PRODUCT, &said, Utc::now())
            .await
            .unwrap();
    }

    let view = get_sources(State(state.clone())).await.unwrap().0;
    let row = |name: &str| view.sources.iter().find(|s| s.name == name).unwrap();
    assert_eq!(row("Patio").missing_sensor, Some(gone.id));
    assert!(row("Patio")
        .last_error
        .as_deref()
        .unwrap()
        .contains("not found in Home Assistant"));
    assert_eq!(
        row("Porch").missing_sensor,
        None,
        "unavailable is not missing"
    );

    // Once it reads again the flag goes with the error.
    let id = source_of(&gone);
    fetch_state::checked(
        &pool,
        id,
        PRODUCT,
        Utc::now() + chrono::Duration::seconds(1),
    )
    .await
    .unwrap();
    let view = get_sources(State(state)).await.unwrap().0;
    let patio = view.sources.iter().find(|s| s.name == "Patio").unwrap();
    assert_eq!(patio.missing_sensor, None);
}
