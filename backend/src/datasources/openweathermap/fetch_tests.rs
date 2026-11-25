//! A whole forecast fetch against a stubbed OpenWeatherMap response.

use super::*;
use chrono::{NaiveDate, TimeZone};

/// Three slots on Sep 22 (8 am, 11 am and 2 pm EDT).
const RESPONSE: &str = r#"{
  "list": [
    {"dt": 1790078400, "main": {"temp": 60.0, "feels_like": 59.0, "humidity": 80.0},
     "weather": [{"id": 500, "main": "Rain", "description": "light rain"}],
     "clouds": {"all": 40.0}, "wind": {"speed": 5.0, "gust": 10.0}, "pop": 0.2,
     "rain": {"3h": 1.0}},
    {"dt": 1790089200, "main": {"temp": 70.0, "feels_like": 70.0, "humidity": 60.0},
     "weather": [{"id": 501, "main": "Rain", "description": "moderate rain"}],
     "clouds": {"all": 90.0}, "wind": {"speed": 7.0}, "pop": 0.7,
     "rain": {"3h": 0.5}, "snow": {"3h": 0.3}},
    {"dt": 1790100000, "main": {"temp": 75.0, "feels_like": 76.0, "humidity": 40.0},
     "weather": [{"id": 800, "main": "Clear", "description": "clear sky"}],
     "clouds": {"all": 0.0}, "wind": {"speed": 9.0, "gust": 15.0}}
  ],
  "city": {"name": "Avondale", "country": "US", "coord": {"lat": 39.8, "lon": -75.8},
           "timezone": -14400}
}"#;

const EASTERN: Clock = Clock::Zone(chrono_tz::America::New_York);

fn client(base: &str) -> OpenWeatherMapClient {
    OpenWeatherMapClient::new().with_base_url(base)
}

// @spec WX-SRC-OWM-001
#[tokio::test]
async fn a_fetch_returns_every_slot_and_a_summary_per_local_day() {
    let base = crate::test_support::stub_http(200, RESPONSE).await;
    let forecast = client(&base)
        .forecast(39.8, -75.8, "test_key", &EASTERN, Utc::now())
        .await
        .unwrap();

    assert_eq!(forecast.hourly.len(), 3);
    let first = &forecast.hourly[0];
    assert_eq!(first.timestamp.to_rfc3339(), "2026-09-22T12:00:00+00:00");
    assert_eq!(
        (first.temp_f, first.humidity_percent, first.precipitation_mm),
        (60.0, 80.0, Some(1.0))
    );
    assert_eq!(
        (
            first.precipitation_prob,
            first.wind_speed_mph,
            first.wind_gust_mph
        ),
        (Some(0.2), 5.0, Some(10.0))
    );
    assert_eq!(first.weather_condition, WeatherCondition::Rain);
    let second = &forecast.hourly[1];
    assert!(
        (second.precipitation_mm.unwrap() - 0.8).abs() < 1e-9,
        "rain + snow in mm"
    );
    assert_eq!(second.wind_gust_mph, None);
    assert_eq!(
        forecast.hourly[2].precipitation_prob,
        Some(0.0),
        "pop defaults to 0"
    );
    assert_eq!(
        forecast.hourly[2].weather_condition,
        WeatherCondition::Clear
    );

    assert_eq!(forecast.daily_summary.len(), 1);
    let day = &forecast.daily_summary[0];
    assert_eq!(day.date, NaiveDate::from_ymd_opt(2026, 9, 22).unwrap());
    assert_eq!((day.high_temp_f, day.low_temp_f), (75.0, 60.0));
    assert_eq!(day.avg_humidity, 60.0);
    assert!((day.total_precipitation_mm.unwrap() - 1.8).abs() < 1e-9);
    assert_eq!(day.max_precipitation_prob, Some(0.7));
    assert_eq!(
        day.dominant_condition,
        WeatherCondition::Rain,
        "most frequent"
    );
    assert_eq!(day.avg_wind_speed_mph, 7.0);
    assert_eq!(day.max_wind_gust_mph, Some(15.0));
    assert_eq!(day.hours_covered, 9, "3 h per point");
}

// @spec WX-SRC-FC-010, WX-SRC-CFG-003
#[tokio::test]
async fn a_fetch_asks_for_the_point_and_with_the_key_it_is_given() {
    let (base, seen) = crate::test_support::stub_http_seeing(200, RESPONSE, "").await;
    let now = Utc.with_ymd_and_hms(2026, 9, 22, 11, 0, 0).unwrap();
    let forecast = client(&base)
        .forecast(41.8781, -87.6298, "the-stored-key", &EASTERN, now)
        .await
        .unwrap();
    let request = seen.await.unwrap();
    let line = request.lines().next().unwrap();
    assert!(line.starts_with("GET /forecast?"), "{line}");
    for part in [
        "lat=41.8781",
        "lon=-87.6298",
        "appid=the-stored-key",
        "units=imperial",
    ] {
        assert!(line.contains(part), "{part} missing from {line}");
    }
    assert_eq!(forecast.fetched_at, now, "stamped with the schedule's now");
    assert_eq!(forecast.provider, ForecastProvider::OpenWeatherMap);
}
