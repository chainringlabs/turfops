//! The Open-Meteo client against responses in the shape recorded from the service on
//! 2026-11-22.

use super::*;
use crate::test_support::{stub_http, stub_http_seeing};
use chrono::TimeZone;

/// Six hours from 2026-11-20 00:00 UTC, with one temperature the model did not give.
const ANALYSIS: &str = r#"{"latitude":39.860916,"longitude":-75.802765,
"utc_offset_seconds":0,"timezone":"GMT","elevation":136.0,
"hourly_units":{"time":"unixtime","temperature_2m":"°F","relative_humidity_2m":"%","precipitation":"mm"},
"hourly":{"time":[1795132800,1795136400,1795140000,1795143600,1795147200,1795150800],
"temperature_2m":[50.1,49.0,45.4,null,44.0,43.5],
"relative_humidity_2m":[44,43,52,55,60,61],
"precipitation":[0.0,0.0,1.2,0.0,0.3,0.0]}}"#;

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 20, hour, minute, 0).unwrap()
}

fn client(url: &str) -> OpenMeteoClient {
    OpenMeteoClient::new(Arc::new(Outside::unpaced())).with_urls(url, url)
}

// @spec WX-SRC-OM-001, WX-SRC-NORM-004
#[test]
fn an_hour_is_stamped_at_its_start_and_takes_the_rain_reported_at_its_end() {
    let hours = parse(ANALYSIS, at(4, 30)).unwrap().hours;
    assert_eq!(hours[0].observed_at, at(0, 0));
    assert_eq!(
        (hours[0].air_temp_f, hours[0].rh_pct, hours[0].precip_mm),
        (Some(50.1), Some(44.0), Some(0.0))
    );
    assert_eq!(hours[1].observed_at, at(1, 0));
    assert_eq!(hours[1].precip_mm, Some(1.2), "reported at 02:00");
    assert_eq!(hours[1].soil_temp_f, [None; 5]);
}

// @spec WX-SRC-OM-001
#[test]
fn an_hour_that_is_not_over_is_left_out() {
    let hours = parse(ANALYSIS, at(4, 30)).unwrap().hours;
    let stamps: Vec<_> = hours.iter().map(|h| h.observed_at).collect();
    assert_eq!(stamps, vec![at(0, 0), at(1, 0), at(2, 0), at(3, 0)]);
    assert_eq!(parse(ANALYSIS, at(5, 0)).unwrap().hours.len(), 5);
}

// @spec WX-SRC-NORM-002
#[test]
fn a_value_the_model_did_not_give_is_empty_and_the_rest_of_the_hour_stands() {
    let hours = parse(ANALYSIS, at(4, 30)).unwrap().hours;
    assert_eq!(
        (hours[3].air_temp_f, hours[3].rh_pct, hours[3].precip_mm),
        (None, Some(55.0), Some(0.3))
    );
}

// @spec WX-SRC-NORM-002
#[test]
fn an_implausible_value_is_empty() {
    let odd = ANALYSIS
        .replace("[50.1,49.0,", "[250.0,49.0,")
        .replace("[44,43,", "[44,143,")
        .replace("[0.0,0.0,1.2,", "[0.0,-1.0,301.0,");
    let hours = parse(&odd, at(4, 30)).unwrap().hours;
    assert_eq!(hours[0].air_temp_f, None);
    assert_eq!(hours[0].precip_mm, None, "negative");
    assert_eq!(hours[1].rh_pct, None);
    assert_eq!(hours[1].precip_mm, None, "more than 300 mm in an hour");
    assert_eq!(hours[1].air_temp_f, Some(49.0));
}

// @spec WX-SRC-NORM-001
#[test]
fn values_are_converted_from_the_units_the_response_declares() {
    let metric = ANALYSIS
        .replace(r#""temperature_2m":"°F""#, r#""temperature_2m":"°C""#)
        .replace("[50.1,49.0,", "[10.0,49.0,")
        .replace(r#""precipitation":"mm""#, r#""precipitation":"inch""#)
        .replace("[0.0,0.0,1.2,", "[0.0,0.0,0.5,");
    let hours = parse(&metric, at(4, 30)).unwrap().hours;
    assert_eq!(hours[0].air_temp_f, Some(50.0));
    assert!((hours[1].precip_mm.unwrap() - 12.7).abs() < 1e-9);
}

// @spec WX-SRC-NORM-001
#[test]
fn a_unit_the_client_does_not_know_is_an_error_not_a_guess() {
    let kelvin = ANALYSIS.replace(r#""temperature_2m":"°F""#, r#""temperature_2m":"K""#);
    assert!(parse(&kelvin, at(4, 30)).unwrap_err().contains('K'));
}

// @spec WX-SRC-OM-003
#[test]
fn the_points_elevation_is_returned_in_feet() {
    let analysis = parse(ANALYSIS, at(4, 30)).unwrap();
    assert!((analysis.elevation_ft.unwrap() - 446.19).abs() < 0.01);
    let unknown = ANALYSIS.replace(r#""elevation":136.0,"#, "");
    assert_eq!(parse(&unknown, at(4, 30)).unwrap().elevation_ft, None);
}

// @spec WX-SRC-OM-001, WX-SRC-OM-002, WX-SRC-REQ-006
#[tokio::test]
async fn the_request_carries_the_point_the_span_and_three_air_variables() {
    let (base, request) = stub_http_seeing(200, ANALYSIS, "").await;
    client(&base)
        .analysis(39.86, -75.79, 30, at(4, 30))
        .await
        .unwrap();
    let request = request.await.unwrap();
    let line = request.lines().next().unwrap();
    for wanted in [
        "latitude=39.86&",
        "longitude=-75.79&",
        "hourly=temperature_2m,relative_humidity_2m,precipitation&",
        "past_days=30&",
    ] {
        assert!(line.contains(wanted), "{wanted} in {line}");
    }
    assert!(!line.contains("soil"), "{line}");
    let parameters = line.matches('=').count();
    assert_eq!(
        parameters, 8,
        "the point, the span and how to format: {line}"
    );
}

// @spec WX-SRC-OM-001
#[tokio::test]
async fn no_more_than_92_days_back_are_asked_for() {
    let (base, request) = stub_http_seeing(200, ANALYSIS, "").await;
    client(&base)
        .analysis(39.86, -75.79, 400, at(4, 30))
        .await
        .unwrap();
    assert!(request.await.unwrap().contains("past_days=92&"));
}

// @spec WX-SRC-ERR-001
#[tokio::test]
async fn a_refusal_or_an_unreadable_body_is_an_outage_of_the_hourly_analysis() {
    let ragged = r#"{"hourly_units":{"temperature_2m":"°F","precipitation":"mm"},
        "hourly":{"time":[1795132800,1795136400],"temperature_2m":[50.1],
        "relative_humidity_2m":[44,43],"precipitation":[0.0,0.0]}}"#;
    for (status, body) in [
        (
            400,
            r#"{"reason":"Latitude must be in range of -90 to 90.","error":true}"#,
        ),
        (404, ""),
        (200, "not json"),
        (200, ragged),
    ] {
        let base = stub_http(status, body).await;
        let err = client(&base)
            .analysis(39.86, -75.79, 2, at(4, 30))
            .await
            .unwrap_err();
        let message = err.owner_message();
        assert!(message.contains("hourly weather analysis"), "{message}");
        assert!(
            !message.contains("Latitude") && !message.contains("127.0.0.1"),
            "{message}"
        );
    }
}
