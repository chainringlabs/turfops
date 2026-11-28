//! The grid client against responses recorded from RCC-ACIS on 2026-11-22.

use super::*;
use crate::test_support::{stub_http, stub_http_seeing};

/// PRISM at Avondale: five published days and today, not yet published.
const PRISM: &str = r#"{"meta":{"lon": -75.79166, "lat": 39.83333},
"data":[["2026-11-17",53.10781,32.37793,0],
["2026-11-18",54.978127,33.563576,0.25],
["2026-11-19",61.728127,37.32617,0],
["2026-11-20",68.675,38.922264,0],
["2026-11-21",66.453125,42.125,0],
["2026-11-22",-999,-999,-999]]}"#;

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}

fn client(url: &str) -> GridClient {
    GridClient::new(Arc::new(Outside::unpaced())).with_url(url)
}

// @spec WX-SRC-GRID-001, WX-SRC-NORM-001
#[test]
fn a_day_carries_maximum_minimum_their_mean_and_rain_in_millimetres() {
    let days = parse(PRISM).unwrap().days;
    assert_eq!(days[0].day, d(17));
    assert_eq!(
        (days[0].air_max_f, days[0].air_min_f),
        (Some(53.10781), Some(32.37793))
    );
    assert!((days[0].air_mean_f.unwrap() - 42.74287).abs() < 1e-5);
    assert_eq!(days[0].precip_mm, Some(0.0));
    assert!((days[1].precip_mm.unwrap() - 6.35).abs() < 1e-9, "0.25 in");
    assert_eq!(
        (days[0].soil_temp_5_f, days[0].soil_temp_10_f),
        (None, None)
    );
}

// @spec WX-SRC-GRID-002
#[test]
fn the_grid_cell_the_values_are_for_is_returned() {
    let got = parse(PRISM).unwrap();
    assert_eq!(
        (got.cell_latitude, got.cell_longitude),
        (39.83333, -75.79166)
    );
}

// @spec WX-SRC-GRID-003
#[test]
fn days_at_the_end_with_no_value_are_not_yet_published() {
    let days = parse(PRISM).unwrap().days;
    assert_eq!(days.len(), 5);
    assert_eq!(days.last().unwrap().day, d(21));
}

// @spec WX-SRC-GRID-003
#[test]
fn a_day_with_no_value_inside_the_span_is_an_error_and_nothing_is_returned() {
    let holed = PRISM.replace(
        r#"["2026-11-19",61.728127,37.32617,0]"#,
        r#"["2026-11-19",-999,-999,-999]"#,
    );
    let err = parse(&holed).unwrap_err();
    assert!(err.contains("2026-11-19"), "{err}");
}

// @spec WX-SRC-NORM-002
#[test]
fn one_missing_value_leaves_the_rest_of_the_day() {
    let partial = PRISM.replace(
        r#"["2026-11-19",61.728127,37.32617,0]"#,
        r#"["2026-11-19",61.728127,-999,0]"#,
    );
    let day = &parse(&partial).unwrap().days[2];
    assert_eq!(
        (day.air_max_f, day.air_min_f, day.air_mean_f),
        (Some(61.728127), None, None)
    );
    assert_eq!(day.precip_mm, Some(0.0));
}

// @spec WX-SRC-GRID-001, WX-SRC-REQ-006
#[tokio::test]
async fn one_request_asks_for_the_point_the_grid_and_the_span_and_nothing_else() {
    let (base, request) = stub_http_seeing(200, PRISM, "").await;
    let got = client(&base)
        .fetch(Grid::Prism, 39.85, -75.78, d(17), d(22))
        .await
        .unwrap();
    assert_eq!(got.days.len(), 5);
    let request = request.await.unwrap();
    let body: serde_json::Value =
        serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "loc": "-75.78,39.85", "sdate": "2026-11-17", "edate": "2026-11-22", "grid": "21",
            "elems": [{"name": "maxt"}, {"name": "mint"}, {"name": "pcpn"}], "meta": "ll"
        })
    );
}

// @spec WX-SRC-GRID-001
#[tokio::test]
async fn the_fallback_grid_is_the_same_request_with_its_own_number() {
    let (base, request) = stub_http_seeing(200, PRISM, "").await;
    client(&base)
        .fetch(Grid::Nrcc, 39.85, -75.78, d(20), d(22))
        .await
        .unwrap();
    assert!(request.await.unwrap().contains(r#""grid":"1""#));
}

// @spec WX-SRC-ERR-001
#[tokio::test]
async fn a_refusal_or_an_unreadable_body_is_an_outage_of_the_grid() {
    for (status, body) in [
        (
            400,
            r#"{"status":"Invalid request.","error":"unknown grid id: 99"}"#,
        ),
        (200, r#"{"error":"no data"}"#),
        (200, "not json"),
    ] {
        let base = stub_http(status, body).await;
        let err = client(&base)
            .fetch(Grid::Prism, 39.85, -75.78, d(17), d(22))
            .await
            .unwrap_err();
        let message = err.owner_message();
        assert!(message.contains("PRISM daily grid"), "{message}");
        assert!(
            !message.contains("unknown grid") && !message.contains("127.0.0.1"),
            "{message}"
        );
    }
}
