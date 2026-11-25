//! `/api/v1/setup/places` with the place-search service stubbed.

use super::{places, PlaceQuery};
use crate::api::applications::db_support::{reply, state};
use crate::datasources::grid::GridClient;
use crate::datasources::open_meteo::OpenMeteoClient;
use crate::datasources::outside::Outside;
use crate::datasources::scan::ScanClient;
use crate::datasources::uscrn::UscrnClient;
use crate::logic::weather_sync::fetcher::{Clients, Fetcher};
use crate::test_support::{stub_http, stub_http_sequence};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use sqlx::PgPool;
use std::sync::Arc;

const NOWHERE: &str = "http://127.0.0.1:1";

/// A fetcher whose place search answers from `base` and whose other clients reach nothing.
pub(crate) fn searching(pool: &PgPool, base: &str) -> Fetcher {
    let outside = Arc::new(Outside::unpaced());
    Fetcher::new(
        pool.clone(),
        Clients {
            uscrn: UscrnClient::new(outside.clone()).with_base(NOWHERE),
            scan: ScanClient::new(outside.clone()).with_base(NOWHERE),
            grid: GridClient::new(outside.clone()).with_url(NOWHERE),
            open_meteo: OpenMeteoClient::new(outside.clone()).with_urls(NOWHERE, base),
            nws: crate::datasources::nws::NwsClient::new(outside).with_base_url(NOWHERE),
            owm: crate::datasources::OpenWeatherMapClient::new().with_base_url(NOWHERE),
        },
    )
}

const FOUND: &str = r#"{"results":[{"id":1,"name":"Avondale","latitude":39.82344,
"longitude":-75.78327,"country_code":"US","admin1":"Pennsylvania"}]}"#;

// @spec SETUP-API-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_search_returns_the_candidate_places(pool: PgPool) {
    let (base, mut requests) = stub_http_sequence(vec![(200, FOUND)]).await;
    let state = state(&pool).await.fetching(searching(&pool, &base));
    let query = PlaceQuery {
        q: Some("19311".into()),
    };
    let found = places(State(state), Query(query)).await.unwrap().0;
    assert_eq!(found.len(), 1);
    assert_eq!(
        (found[0].name.as_str(), found[0].state.as_deref()),
        ("Avondale", Some("Pennsylvania"))
    );
    assert!(requests.recv().await.unwrap().contains("name=19311&"));
}

// @spec SETUP-FAIL-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_search_that_fails_says_to_enter_coordinates(pool: PgPool) {
    let base = stub_http(500, "upstream exploded").await;
    let state = state(&pool).await.fetching(searching(&pool, &base));
    let query = PlaceQuery {
        q: Some("Avondale".into()),
    };
    let err = places(State(state), Query(query)).await.unwrap_err();
    assert_eq!(
        reply(err).await,
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Place search is unavailable; enter coordinates instead.".to_string()
        )
    );
}
