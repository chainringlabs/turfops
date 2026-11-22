//! The Open-Meteo forecast against a response in the shape recorded on 2026-11-22.

use super::*;
use crate::datasources::outside::Outside;
use crate::test_support::{stub_http, stub_http_seeing};
use chrono::TimeZone;
use std::sync::Arc;

/// Five stamps from 2026-11-22 12:00 UTC.
const FORECAST: &str = r#"{"latitude":39.86,"longitude":-75.8,"elevation":136.0,
"hourly_units":{"time":"unixtime","temperature_2m":"°F","relative_humidity_2m":"%",
 "apparent_temperature":"°F","precipitation":"mm","precipitation_probability":"%",
 "wind_speed_10m":"mp/h","wind_gusts_10m":"mp/h","cloud_cover":"%","weather_code":"wmo code"},
"hourly":{"time":[1795348800,1795352400,1795356000,1795359600,1795363200],
"temperature_2m":[52.8,51.5,50.0,null,49.0],
"relative_humidity_2m":[57,68,70,72,75],
"apparent_temperature":[49.0,48.9,47.0,46.0,45.0],
"precipitation":[9.9,0.0,1.4,0.2,null],
"precipitation_probability":[99,1,80,null,15],
"wind_speed_10m":[2.5,1.1,3.0,3.5,4.0],
"wind_gusts_10m":[4.0,4.5,9.0,10.0,11.0],
"cloud_cover":[99,99,100,100,80],
"weather_code":[3,3,61,61,2]}}"#;

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 22, hour, minute, 0).unwrap()
}

fn client(url: &str) -> OpenMeteoClient {
    OpenMeteoClient::new(Arc::new(Outside::unpaced())).with_urls(url, url)
}

// @spec WX-SRC-FC-005, WX-SRC-FC-002
#[test]
fn an_hour_takes_instants_from_its_start_and_sums_from_its_end() {
    let points = parse(FORECAST, at(11, 0)).unwrap();
    let first = &points[0];
    assert_eq!((first.timestamp, first.slot_hours), (at(12, 0), 1));
    assert_eq!(
        (first.temp_f, first.humidity_percent, first.wind_speed_mph),
        (52.8, 57.0, 2.5)
    );
    assert_eq!(
        (first.feels_like_f, first.cloud_cover_percent),
        (Some(49.0), Some(99.0))
    );
    assert_eq!(first.weather_condition, WeatherCondition::Clouds);
    assert_eq!(
        (
            first.precipitation_mm,
            first.precipitation_prob,
            first.wind_gust_mph
        ),
        (Some(0.0), Some(0.01), Some(4.5)),
        "stamped at 13:00"
    );
    let second = &points[1];
    assert_eq!(
        (second.precipitation_mm, second.precipitation_prob),
        (Some(1.4), Some(0.8))
    );
}

// @spec WX-SRC-FC-005
#[test]
fn the_forecast_starts_at_the_hour_in_progress_and_ends_at_the_last_full_hour() {
    let starts = |now| -> Vec<_> {
        parse(FORECAST, now)
            .unwrap()
            .iter()
            .map(|p| p.timestamp)
            .collect()
    };
    assert_eq!(starts(at(13, 20)), vec![at(13, 0), at(14, 0)]);
    assert_eq!(starts(at(14, 0)), vec![at(14, 0)]);
}

// @spec WX-SRC-FC-003
#[test]
fn a_quantity_not_forecast_is_empty_and_an_hour_without_temperature_is_left_out() {
    let points = parse(FORECAST, at(11, 0)).unwrap();
    let starts: Vec<_> = points.iter().map(|p| p.timestamp).collect();
    assert_eq!(
        starts,
        vec![at(12, 0), at(13, 0), at(14, 0)],
        "15:00 has none"
    );
    assert_eq!(points[2].precipitation_prob, None);
    assert_eq!(points[2].precipitation_mm, Some(0.2));
}

// @spec WX-SRC-NORM-001
#[test]
fn metric_units_are_converted() {
    let metric = FORECAST
        .replace(r#""temperature_2m":"°F""#, r#""temperature_2m":"°C""#)
        .replace("[52.8,51.5,", "[10.0,51.5,")
        .replace(r#""wind_speed_10m":"mp/h""#, r#""wind_speed_10m":"km/h""#)
        .replace("[2.5,1.1,", "[10.0,1.1,");
    let first = &parse(&metric, at(11, 0)).unwrap()[0];
    assert_eq!(first.temp_f, 50.0);
    assert!((first.wind_speed_mph - 6.21371).abs() < 1e-9);
}

// @spec WX-SRC-FC-005, WX-SRC-FC-008, WX-SRC-OM-002, WX-SRC-REQ-006
#[tokio::test]
async fn the_request_carries_the_point_and_no_soil_variable() {
    let (base, request) = stub_http_seeing(200, FORECAST, "").await;
    let clock = Clock::Zone(chrono_tz::America::New_York);
    let forecast = client(&base)
        .forecast(39.86, -75.79, &clock, at(11, 0))
        .await
        .unwrap();
    assert_eq!(forecast.provider, ForecastProvider::OpenMeteo);
    assert_eq!(forecast.fetched_at, at(11, 0));
    assert_eq!(forecast.daily_summary.len(), 1);
    assert_eq!(forecast.daily_summary[0].hours_covered, 3);
    let request = request.await.unwrap();
    let line = request.lines().next().unwrap();
    assert!(line.contains("latitude=39.86&longitude=-75.79&"), "{line}");
    assert!(line.contains("forecast_days=16"), "{line}");
    assert!(
        !line.contains("soil") && !line.contains("past_days"),
        "{line}"
    );
}

// @spec WX-SRC-ERR-001
#[tokio::test]
async fn a_refusal_or_an_unreadable_body_is_an_outage_of_the_forecast() {
    for (status, body) in [
        (429, r#"{"reason":"quota exceeded","error":true}"#),
        (200, "{}"),
    ] {
        let base = stub_http(status, body).await;
        let message = client(&base)
            .forecast(39.86, -75.79, &Clock::System, at(11, 0))
            .await
            .unwrap_err()
            .owner_message();
        assert!(
            message.contains("weather forecast is unavailable"),
            "{message}"
        );
        assert!(!message.contains("quota") && !message.contains("127.0.0.1"));
    }
}
