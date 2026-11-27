//! An OpenWeatherMap slot's values are checked like every other source's (WX-SRC-NORM-002).

use super::*;

/// A plausible slot; each test changes one value.
const SLOT: &str = r#"{"dt": 1790078400,
 "main": {"temp": 60.0, "feels_like": 59.0, "humidity": 80.0},
 "weather": [{"id": 500, "main": "Rain", "description": "light rain"}],
 "clouds": {"all": 40.0}, "wind": {"speed": 5.0, "gust": 10.0}, "pop": 0.2,
 "rain": {"3h": 1.0}}"#;

fn slot(from: &str, to: &str) -> Option<ForecastPoint> {
    assert!(SLOT.contains(from), "{from} is not in the slot");
    let item: OwmForecastItem = serde_json::from_str(&SLOT.replace(from, to)).unwrap();
    convert_forecast_item(&item)
}

// @spec WX-SRC-NORM-002
#[test]
fn a_plausible_slot_keeps_every_value() {
    let point = slot("", "").unwrap();
    assert_eq!(
        (point.temp_f, point.humidity_percent, point.wind_speed_mph),
        (60.0, 80.0, 5.0)
    );
    assert_eq!(
        (
            point.feels_like_f,
            point.precipitation_mm,
            point.precipitation_prob
        ),
        (Some(59.0), Some(1.0), Some(0.2))
    );
    assert_eq!(
        (point.wind_gust_mph, point.cloud_cover_percent),
        (Some(10.0), Some(40.0))
    );
}

// @spec WX-SRC-NORM-002, WX-SRC-FC-003
#[test]
fn a_slot_with_an_implausible_temperature_humidity_or_wind_is_not_returned() {
    for (from, to) in [
        (r#""temp": 60.0"#, r#""temp": 151.0"#),
        (r#""temp": 60.0"#, r#""temp": -81.0"#),
        (r#""humidity": 80.0"#, r#""humidity": 101.0"#),
        (r#""humidity": 80.0"#, r#""humidity": -1.0"#),
        (r#""speed": 5.0"#, r#""speed": -1.0"#),
    ] {
        assert!(slot(from, to).is_none(), "{to}");
    }
}

// @spec WX-SRC-NORM-002
#[test]
fn an_implausible_optional_value_is_emptied_and_the_rest_of_the_slot_kept() {
    let feels = slot(r#""feels_like": 59.0"#, r#""feels_like": 151.0"#).unwrap();
    assert_eq!((feels.feels_like_f, feels.temp_f), (None, 60.0));

    // A 3-hour slot may hold up to 300 mm for each of its hours.
    let flood = slot(r#""3h": 1.0"#, r#""3h": 901.0"#).unwrap();
    assert_eq!(flood.precipitation_mm, None);
    let heavy = slot(r#""3h": 1.0"#, r#""3h": 900.0"#).unwrap();
    assert_eq!(heavy.precipitation_mm, Some(900.0));
    let negative = slot(r#""3h": 1.0"#, r#""3h": -1.0"#).unwrap();
    assert_eq!(negative.precipitation_mm, None);

    let pop = slot(r#""pop": 0.2"#, r#""pop": 1.5"#).unwrap();
    assert_eq!(pop.precipitation_prob, None);
    let clouds = slot(r#""all": 40.0"#, r#""all": 140.0"#).unwrap();
    assert_eq!(clouds.cloud_cover_percent, None);
    let gust = slot(r#""gust": 10.0"#, r#""gust": -2.0"#).unwrap();
    assert_eq!((gust.wind_gust_mph, gust.wind_speed_mph), (None, 5.0));
}

// @spec WX-SRC-NORM-004
#[test]
fn a_slot_whose_time_is_not_a_time_is_not_returned() {
    assert!(slot(r#""dt": 1790078400"#, r#""dt": 9223372036854775807"#).is_none());
}
