//! Conditions and the day window.

use super::*;

#[test]
fn weather_condition_from_owm_id() {
    assert_eq!(
        WeatherCondition::from_owm_id(200),
        WeatherCondition::Thunderstorm
    );
    assert_eq!(WeatherCondition::from_owm_id(500), WeatherCondition::Rain);
    assert_eq!(WeatherCondition::from_owm_id(800), WeatherCondition::Clear);
    assert_eq!(WeatherCondition::from_owm_id(801), WeatherCondition::Clouds);
    assert_eq!(WeatherCondition::from_owm_id(600), WeatherCondition::Snow);
}

fn summary(date: NaiveDate) -> DailyForecast {
    DailyForecast {
        date,
        high_temp_f: 70.0,
        low_temp_f: 50.0,
        avg_humidity: 50.0,
        total_precipitation_mm: Some(0.0),
        max_precipitation_prob: Some(0.0),
        dominant_condition: WeatherCondition::Clear,
        avg_wind_speed_mph: 0.0,
        max_wind_gust_mph: None,
        hours_covered: 24,
    }
}

// @spec WX-SRC-OWM-004
#[test]
fn next_days_counts_from_the_given_local_today() {
    let d = |m, day| NaiveDate::from_ymd_opt(2025, m, day).unwrap();
    let forecast = WeatherForecast {
        fetched_at: Utc::now(),
        provider: ForecastProvider::OpenWeatherMap,
        hourly: vec![],
        daily_summary: (1..=6).map(|day| summary(d(6, day))).collect(),
    };
    let dates: Vec<_> = forecast
        .next_days(d(6, 2), 2)
        .iter()
        .map(|s| s.date)
        .collect();
    assert_eq!(dates, vec![d(6, 1), d(6, 2), d(6, 3), d(6, 4)]);
}
