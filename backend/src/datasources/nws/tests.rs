//! The National Weather Service client against responses in the shape recorded from
//! `api.weather.gov` on 2026-11-22.

use super::parse::duration_hours;
use super::*;
use crate::models::forecast::WeatherCondition;
use crate::test_support::stub_http_sequence;
use chrono::TimeZone;

const POINT: &str = r#"{"properties":{"gridId":"PHI","gridX":29,"gridY":71,
"forecastGridData":"https://elsewhere.example/gridpoints/XXX/1,1"}}"#;

/// From 2026-11-22 15:00 UTC: ten hours of temperature, rain amounts for the first six.
const GRID: &str = r#"{"properties":{
"temperature":{"uom":"wmoUnit:degC","values":[
 {"validTime":"2026-11-22T15:00:00+00:00/PT1H","value":15.0},
 {"validTime":"2026-11-22T16:00:00+00:00/PT2H","value":20.0},
 {"validTime":"2026-11-22T18:00:00+00:00/PT7H","value":10.0}]},
"relativeHumidity":{"uom":"wmoUnit:percent","values":[
 {"validTime":"2026-11-22T15:00:00+00:00/PT9H","value":86}]},
"apparentTemperature":{"uom":"wmoUnit:degC","values":[
 {"validTime":"2026-11-22T15:00:00+00:00/PT1H","value":10.0}]},
"quantitativePrecipitation":{"uom":"wmoUnit:mm","values":[
 {"validTime":"2026-11-22T15:00:00+00:00/PT6H","value":6.0}]},
"probabilityOfPrecipitation":{"uom":"wmoUnit:percent","values":[
 {"validTime":"2026-11-22T15:00:00+00:00/PT6H","value":80},
 {"validTime":"2026-11-22T21:00:00+00:00/PT6H","value":20}]},
"windSpeed":{"uom":"wmoUnit:km_h-1","values":[
 {"validTime":"2026-11-22T15:00:00+00:00/P1D","value":10.0}]},
"windGust":{"uom":"wmoUnit:km_h-1","values":[
 {"validTime":"2026-11-22T15:00:00+00:00/PT1H","value":20.0}]},
"skyCover":{"uom":"wmoUnit:percent","values":[
 {"validTime":"2026-11-22T15:00:00+00:00/PT8H","value":83},
 {"validTime":"2026-11-22T23:00:00+00:00/PT1H","value":20}]},
"weather":{"values":[
 {"validTime":"2026-11-22T15:00:00+00:00/PT2H","value":[
  {"coverage":"likely","weather":"rain_showers","intensity":"light","attributes":[]}]},
 {"validTime":"2026-11-22T17:00:00+00:00/PT1H","value":[
  {"coverage":"slight_chance","weather":"thunderstorms","intensity":null,"attributes":[]}]},
 {"validTime":"2026-11-22T18:00:00+00:00/PT7H","value":[
  {"coverage":null,"weather":null,"intensity":null,"attributes":[]}]}]}
}}"#;

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 22, hour, minute, 0).unwrap()
}

fn points() -> Vec<crate::models::ForecastPoint> {
    parse(GRID, at(14, 0)).unwrap()
}

// @spec WX-SRC-FC-006, WX-SRC-FC-002, WX-SRC-NORM-001
#[test]
fn a_value_stands_for_every_hour_of_its_span() {
    let points = points();
    let starts: Vec<_> = points.iter().map(|p| p.timestamp).collect();
    let expected: Vec<_> = (15..24).map(|hour| at(hour, 0)).collect();
    assert_eq!(starts, expected, "the tenth hour has no humidity");
    assert!(points.iter().all(|p| p.slot_hours == 1));
    let temps: Vec<_> = points[..4].iter().map(|p| p.temp_f).collect();
    assert_eq!(temps, vec![59.0, 68.0, 68.0, 50.0]);
    assert!((points[0].wind_speed_mph - 6.21371).abs() < 1e-9);
    assert_eq!(points[0].humidity_percent, 86.0);
}

// @spec WX-SRC-FC-006
#[test]
fn a_rain_amount_is_divided_evenly_across_the_hours_of_its_span() {
    let points = points();
    for point in &points[..6] {
        assert_eq!(point.precipitation_mm, Some(1.0), "{}", point.timestamp);
        assert_eq!(point.precipitation_prob, Some(0.8));
    }
}

// @spec WX-SRC-FC-003
#[test]
fn past_the_last_rain_amount_the_amount_is_empty_and_the_probability_stands() {
    let later = &points()[6];
    assert_eq!(later.timestamp, at(21, 0));
    assert_eq!(later.precipitation_mm, None);
    assert_eq!(later.precipitation_prob, Some(0.2));
    assert_eq!((later.wind_gust_mph, later.feels_like_f), (None, None));
    assert_eq!(points()[0].feels_like_f, Some(50.0));
}

// @spec WX-SRC-FC-009
#[test]
fn the_condition_is_the_weather_when_it_is_more_than_a_chance_else_the_sky() {
    let conditions: Vec<_> = points().iter().map(|p| p.weather_condition).collect();
    assert_eq!(conditions[0], WeatherCondition::Rain);
    assert_eq!(conditions[2], WeatherCondition::Clouds, "a slight chance");
    assert_eq!(conditions[3], WeatherCondition::Clouds, "83% sky cover");
    assert_eq!(conditions[8], WeatherCondition::Clear, "20% sky cover");
    let no_sky = GRID.replace(r#""skyCover""#, r#""otherCover""#);
    let bare = parse(&no_sky, at(14, 0)).unwrap();
    assert_eq!(bare[3].weather_condition, WeatherCondition::Other);
}

// @spec WX-SRC-FC-006
#[test]
fn the_forecast_starts_at_the_hour_in_progress() {
    let first = |now| parse(GRID, now).unwrap()[0].timestamp;
    assert_eq!(first(at(16, 59)), at(16, 0));
    assert_eq!(first(at(17, 0)), at(17, 0));
}

// @spec WX-SRC-FC-006
#[test]
fn a_span_is_days_and_hours() {
    assert_eq!(duration_hours("PT1H"), Some(1));
    assert_eq!(duration_hours("P1D"), Some(24));
    assert_eq!(duration_hours("P7DT10H"), Some(178));
    assert_eq!(duration_hours("PT30M"), None);
    assert_eq!(duration_hours("6H"), None);
}

// @spec WX-SRC-NORM-001, WX-SRC-ERR-001
#[test]
fn an_unknown_unit_or_an_unreadable_span_is_an_error_not_a_guess() {
    let kelvin = GRID.replace("wmoUnit:degC", "wmoUnit:K");
    assert!(parse(&kelvin, at(14, 0)).unwrap_err().contains("wmoUnit:K"));
    let odd = GRID.replace("15:00:00+00:00/PT9H", "15:00:00+00:00/PT9000H");
    assert!(parse(&odd, at(14, 0)).is_err());
}

fn client(base: &str) -> NwsClient {
    NwsClient::new(Arc::new(Outside::unpaced())).with_base_url(base)
}

// @spec WX-SRC-FC-007, WX-SRC-FC-008, WX-SRC-REQ-006
#[tokio::test]
async fn the_cell_is_asked_for_once_and_its_forecast_is_fetched_from_the_service() {
    let (base, mut requests) =
        stub_http_sequence(vec![(200, POINT), (200, GRID), (200, GRID)]).await;
    let client = client(&base);
    let clock = Clock::Zone(chrono_tz::America::New_York);
    let forecast = client
        .forecast(39.861234, -75.79, &clock, at(14, 0))
        .await
        .unwrap();
    assert_eq!(forecast.provider, ForecastProvider::NationalWeatherService);
    assert_eq!(forecast.hourly.len(), 9);
    assert_eq!(forecast.daily_summary[0].hours_covered, 9);
    assert_eq!(forecast.daily_summary[0].total_precipitation_mm, None);
    client
        .forecast(39.861234, -75.79, &clock, at(14, 0))
        .await
        .unwrap();
    let lines = [
        requests.recv().await.unwrap(),
        requests.recv().await.unwrap(),
        requests.recv().await.unwrap(),
    ];
    assert_eq!(lines[0], "GET /points/39.8612,-75.7900 HTTP/1.1");
    assert_eq!(lines[1], "GET /gridpoints/PHI/29,71 HTTP/1.1");
    assert_eq!(lines[2], lines[1], "the cell is not asked for again");
}

// @spec WX-SRC-ERR-001
#[tokio::test]
async fn a_point_the_service_does_not_cover_is_an_outage_of_the_forecast() {
    let refused = r#"{"title":"Data Unavailable For Requested Point","status":404}"#;
    let hostile = r#"{"properties":{"gridId":"../x","gridX":1,"gridY":1}}"#;
    for answers in [
        vec![(404, refused)],
        vec![(200, hostile)],
        vec![(200, POINT), (503, "busy")],
        vec![(200, POINT), (200, "{}")],
    ] {
        let (base, _requests) = stub_http_sequence(answers).await;
        let message = client(&base)
            .forecast(10.0, -75.0, &Clock::System, at(14, 0))
            .await
            .unwrap_err()
            .owner_message();
        assert!(
            message.contains("weather forecast is unavailable"),
            "{message}"
        );
        assert!(!message.contains("Requested Point") && !message.contains("127.0.0.1"));
    }
}
