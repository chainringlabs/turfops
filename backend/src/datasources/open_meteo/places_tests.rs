//! The place search against responses in the shape recorded from Open-Meteo on 2026-11-22.

use super::*;
use crate::datasources::outside::Outside;
use crate::test_support::{stub_http, stub_http_seeing};
use std::sync::Arc;

const FOUND: &str = r#"{"results":[
{"id":4556349,"name":"Avondale","latitude":39.82344,"longitude":-75.78327,"elevation":82.0,
 "country_code":"US","timezone":"America/New_York","postcodes":["19311"],
 "country":"United States","admin1":"Pennsylvania","admin2":"Chester"},
{"id":1,"name":"Avondale","latitude":-36.9,"longitude":174.7,"country_code":"NZ","admin1":"Auckland"},
{"id":2,"name":"Avondale","latitude":33.43,"longitude":-112.35,"country_code":"US"}],
"generationtime_ms":0.24}"#;

fn client(url: &str) -> OpenMeteoClient {
    OpenMeteoClient::new(Arc::new(Outside::unpaced())).with_urls(url, url)
}

// @spec WX-SRC-OM-004
#[test]
fn candidates_are_the_places_in_the_united_states_with_their_state_and_coordinates() {
    let places = parse(FOUND).unwrap();
    assert_eq!(
        places,
        vec![
            Place {
                name: "Avondale".into(),
                state: Some("Pennsylvania".into()),
                latitude: 39.82344,
                longitude: -75.78327,
            },
            Place {
                name: "Avondale".into(),
                state: None,
                latitude: 33.43,
                longitude: -112.35,
            },
        ]
    );
}

// @spec WX-SRC-OM-004
#[test]
fn a_search_that_matches_nothing_is_an_empty_list() {
    assert_eq!(parse(r#"{"generationtime_ms":0.13}"#), Ok(vec![]));
}

// @spec WX-SRC-OM-004
#[tokio::test]
async fn the_request_carries_the_typed_text_and_nothing_else_about_the_lawn() {
    let (base, request) = stub_http_seeing(200, FOUND, "").await;
    client(&base).places("  West Chester, PA ").await.unwrap();
    let request = request.await.unwrap();
    let line = request.lines().next().unwrap();
    assert!(line.contains("name=West+Chester%2C+PA&"), "{line}");
    assert!(line.contains("countryCode=US"), "{line}");
    assert!(
        !line.contains("latitude") && !line.contains("longitude"),
        "{line}"
    );
    assert_eq!(line.matches('=').count(), 5, "{line}");
}

// @spec WX-SRC-OM-004
#[tokio::test]
async fn blank_text_finds_nothing_and_asks_nobody() {
    let places = client("http://127.0.0.1:1").places("   ").await.unwrap();
    assert!(places.is_empty());
}

// @spec WX-SRC-ERR-001
#[tokio::test]
async fn a_failed_search_is_an_outage_of_the_place_search() {
    for (status, body) in [(500, "upstream exploded"), (200, "not json")] {
        let base = stub_http(status, body).await;
        let message = client(&base)
            .places("19311")
            .await
            .unwrap_err()
            .owner_message();
        assert!(message.contains("Place search is unavailable"), "{message}");
        assert!(!message.contains("exploded") && !message.contains("127.0.0.1"));
    }
}
