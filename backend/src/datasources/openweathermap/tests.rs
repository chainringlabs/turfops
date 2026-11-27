use super::*;
use chrono::NaiveDate;

fn eastern() -> Clock {
    Clock::Zone(chrono_tz::America::New_York)
}

/// The forecast's daily summaries for `points`, on Eastern days.
fn days(points: &[ForecastPoint]) -> Vec<crate::models::DailyForecast> {
    WeatherForecast::new(
        ForecastProvider::OpenWeatherMap,
        Utc::now(),
        points.to_vec(),
        &eastern(),
    )
    .daily_summary
}

fn point(utc: &str, temp_f: f64) -> ForecastPoint {
    ForecastPoint {
        timestamp: utc.parse().unwrap(),
        slot_hours: 3,
        temp_f,
        feels_like_f: Some(temp_f),
        humidity_percent: 50.0,
        precipitation_mm: Some(0.0),
        precipitation_prob: Some(0.0),
        wind_speed_mph: 5.0,
        wind_gust_mph: None,
        cloud_cover_percent: Some(0.0),
        weather_condition: WeatherCondition::default(),
    }
}

// @spec WX-SRC-OWM-002
#[test]
fn days_are_local_and_partial_days_are_marked() {
    // Fetched mid-afternoon Sep 21 (EDT): two points left today, then a full Sep 22.
    let mut hourly = vec![
        point("2026-09-21T21:00:00Z", 78.0), // 5 pm Sep 21 local
        point("2026-09-22T00:00:00Z", 70.0), // 8 pm Sep 21 local — still the 21st
    ];
    for (i, temp) in [62.0, 58.0, 57.0, 66.0, 76.0, 80.0, 77.0, 69.0]
        .iter()
        .enumerate()
    {
        let hour = 3 + 3 * i as i64; // 03:00Z Sep 22 (11 pm local Sep 21)… 00:00Z Sep 23
        let ts: DateTime<Utc> = "2026-09-22T00:00:00Z".parse().unwrap();
        let mut p = point("2026-09-22T00:00:00Z", *temp);
        p.timestamp = ts + chrono::Duration::hours(hour);
        hourly.push(p);
    }

    let days = days(&hourly);
    let sep21 = &days[0];
    assert_eq!(sep21.date, NaiveDate::from_ymd_opt(2026, 9, 21).unwrap());
    assert_eq!(sep21.hours_covered, 9); // 5 pm, 8 pm, 11 pm
    assert!(!sep21.is_full_day());

    let sep22 = &days[1];
    assert_eq!(sep22.date, NaiveDate::from_ymd_opt(2026, 9, 22).unwrap());
    // The afternoon high and the dawn low of the same local day land together.
    assert_eq!(sep22.high_temp_f, 80.0);
    assert_eq!(sep22.low_temp_f, 57.0);
    assert_eq!(sep22.hours_covered, 21);
    assert!(sep22.is_full_day());
}

// @spec WX-SRC-OWM-002
#[test]
fn days_follow_daylight_saving_not_a_fixed_offset() {
    // DST ends 2026-11-01 06:00 UTC. 04:30 UTC on Nov 2 is 11:30 pm EST on Nov 1; a fixed
    // EDT offset (what OWM's `city.timezone` reported at fetch time) would call it Nov 2.
    let days = days(&[
        point("2026-11-01T22:00:00Z", 60.0), // 5 pm EST Nov 1
        point("2026-11-02T04:30:00Z", 50.0), // 11:30 pm EST Nov 1
    ]);
    assert_eq!(days.len(), 1);
    assert_eq!(days[0].date, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
}

const FAKE_KEY: &str = "owm-secret-key-0123456789";

/// A client whose requests fail at the network level (nothing listens on port 1), the
/// failure whose reqwest error carries the request URL.
fn unreachable_client() -> OpenWeatherMapClient {
    OpenWeatherMapClient::new(std::sync::Arc::new(
        crate::datasources::outside::Outside::unpaced(),
    ))
    .with_base_url("http://127.0.0.1:1")
}

async fn fetch(client: &OpenWeatherMapClient, key: &str) -> Result<WeatherForecast> {
    client
        .forecast(39.8561, -75.7872, key, &eastern(), Utc::now())
        .await
}

// @spec WX-SRC-OWM-005
#[tokio::test]
async fn a_failed_forecast_fetch_never_carries_the_api_key() {
    let err = fetch(&unreachable_client(), FAKE_KEY).await.unwrap_err();
    let text = format!("{err} {err:?}");
    assert!(!text.contains(FAKE_KEY), "API key leaked: {text}");
}

// @spec WX-SRC-OWM-006
#[tokio::test]
async fn a_rejected_forecast_request_keeps_the_upstream_body_out_of_the_owner_message() {
    let base =
        crate::test_support::stub_http(401, r#"{"cod":401,"message":"Invalid API key secret"}"#)
            .await;
    let client = OpenWeatherMapClient::new(std::sync::Arc::new(
        crate::datasources::outside::Outside::unpaced(),
    ))
    .with_base_url(&base);
    let msg = fetch(&client, "test_key")
        .await
        .unwrap_err()
        .owner_message();
    assert!(!msg.contains("Invalid API key secret"), "{msg}");
    assert!(!msg.contains("401"), "no status code: {msg}");
    assert!(msg.contains("forecast"), "{msg}");
}
