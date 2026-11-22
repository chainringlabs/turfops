//! Daily summaries of points whose slots differ in length and in what they carry.

use super::*;
use chrono::TimeZone;

fn eastern() -> Clock {
    Clock::Zone(chrono_tz::America::New_York)
}

/// A point starting at `hour` UTC on 2026-07-15 (Eastern is four hours behind).
fn point(hour: u32, slot_hours: u32) -> ForecastPoint {
    ForecastPoint {
        timestamp: Utc.with_ymd_and_hms(2026, 7, 15, hour, 0, 0).unwrap(),
        slot_hours,
        temp_f: 70.0,
        feels_like_f: None,
        humidity_percent: 50.0,
        precipitation_mm: Some(0.0),
        precipitation_prob: Some(0.0),
        wind_speed_mph: 5.0,
        wind_gust_mph: None,
        cloud_cover_percent: None,
        weather_condition: WeatherCondition::Clear,
    }
}

fn one_day(points: Vec<ForecastPoint>) -> DailyForecast {
    let forecast = WeatherForecast::new(
        ForecastProvider::NationalWeatherService,
        Utc::now(),
        points,
        &eastern(),
    );
    assert_eq!(forecast.daily_summary.len(), 1);
    forecast.daily_summary.into_iter().next().unwrap()
}

// @spec WX-SRC-OWM-001, WX-SRC-FC-002
#[test]
fn hours_covered_is_the_sum_of_the_slot_lengths() {
    let hourly = one_day((6..24).map(|hour| point(hour, 1)).collect());
    assert_eq!(hourly.hours_covered, 18);
    assert!(hourly.is_full_day());
    let three_hourly = one_day(vec![point(6, 3), point(9, 3)]);
    assert_eq!(three_hourly.hours_covered, 6);
}

// @spec WX-SRC-OWM-001, WX-SRC-FC-003
#[test]
fn the_days_total_is_empty_unless_every_slot_carries_an_amount() {
    let mut points: Vec<_> = (6..12).map(|hour| point(hour, 1)).collect();
    points[0].precipitation_mm = Some(1.5);
    points[1].precipitation_mm = Some(0.5);
    assert_eq!(one_day(points.clone()).total_precipitation_mm, Some(2.0));
    points[5].precipitation_mm = None;
    assert_eq!(one_day(points).total_precipitation_mm, None);
}

// @spec WX-SRC-OWM-001, WX-SRC-FC-003
#[test]
fn the_highest_probability_is_over_the_slots_that_carry_one() {
    let mut points: Vec<_> = (6..9).map(|hour| point(hour, 1)).collect();
    points[0].precipitation_prob = Some(0.3);
    points[1].precipitation_prob = None;
    points[2].precipitation_prob = Some(0.6);
    assert_eq!(one_day(points.clone()).max_precipitation_prob, Some(0.6));
    for point in &mut points {
        point.precipitation_prob = None;
    }
    assert_eq!(one_day(points).max_precipitation_prob, None);
}

// @spec WX-SRC-OWM-001
#[test]
fn means_and_the_condition_weigh_each_slot_by_its_length() {
    let mut long = point(6, 3);
    long.humidity_percent = 90.0;
    long.wind_speed_mph = 12.0;
    long.weather_condition = WeatherCondition::Rain;
    let day = one_day(vec![point(4, 1), point(5, 1), long]);
    assert_eq!(day.avg_humidity, (50.0 + 50.0 + 3.0 * 90.0) / 5.0);
    assert_eq!(day.avg_wind_speed_mph, (5.0 + 5.0 + 3.0 * 12.0) / 5.0);
    assert_eq!(day.dominant_condition, WeatherCondition::Rain);
}

// @spec WX-SRC-OWM-004
#[test]
fn a_slot_with_no_amount_adds_nothing_to_the_rain_expected() {
    let now = Utc.with_ymd_and_hms(2026, 7, 15, 6, 0, 0).unwrap();
    let mut points: Vec<_> = (6..10).map(|hour| point(hour, 1)).collect();
    points[0].precipitation_mm = Some(2.0);
    points[1].precipitation_mm = None;
    points[1].precipitation_prob = None;
    let forecast = |points: Vec<ForecastPoint>| {
        WeatherForecast::new(
            ForecastProvider::NationalWeatherService,
            now,
            points,
            &eastern(),
        )
    };
    let known = forecast(points.clone());
    assert_eq!(
        known
            .rain_expected_within(now, 12, 2.0)
            .unwrap()
            .expected_mm,
        2.0
    );
    assert!(known.rain_expected_within(now, 12, 2.5).is_none());
    points[2].precipitation_mm = None;
    points[2].precipitation_prob = Some(0.7);
    let likely = forecast(points).rain_expected_within(now, 12, 2.5).unwrap();
    assert_eq!((likely.expected_mm, likely.max_probability), (2.0, 0.7));
}

// @spec WX-SRC-FC-009
#[test]
fn a_wmo_weather_code_maps_to_a_shared_condition() {
    for (code, condition) in [
        (0, WeatherCondition::Clear),
        (3, WeatherCondition::Clouds),
        (48, WeatherCondition::Fog),
        (53, WeatherCondition::Drizzle),
        (63, WeatherCondition::Rain),
        (81, WeatherCondition::Rain),
        (73, WeatherCondition::Snow),
        (95, WeatherCondition::Thunderstorm),
        (4, WeatherCondition::Other),
    ] {
        assert_eq!(WeatherCondition::from_wmo_code(code), condition, "{code}");
    }
}
