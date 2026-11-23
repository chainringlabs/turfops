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
