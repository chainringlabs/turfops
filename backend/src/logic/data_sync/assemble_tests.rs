//! The summary from the record's answers.

use super::*;
use crate::logic::weather_record::live::{LiveReading, LiveValue};
use crate::models::weather_basis::{Basis, BasisKind, Figure};
use crate::models::{BasisStanding, ForecastProvider, Trend};
use chrono::TimeZone;

fn at(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 24, hour, 0, 0).unwrap()
}

fn now() -> DateTime<Utc> {
    at(18)
}

fn station() -> Basis {
    Basis {
        kind: BasisKind::Station,
        source: "PA Avondale 2 N".into(),
        distance_miles: Some(2.1),
        approximate: false,
    }
}

fn sensor() -> Basis {
    Basis {
        kind: BasisKind::Sensor,
        source: "Patio temperature".into(),
        distance_miles: None,
        approximate: false,
    }
}

fn analysis() -> Basis {
    Basis {
        kind: BasisKind::Estimate,
        source: "Hourly analysis".into(),
        distance_miles: None,
        approximate: true,
    }
}

fn live(values: &[(Input, f64, DateTime<Utc>, Basis)]) -> LiveReading {
    LiveReading {
        values: values
            .iter()
            .map(|(input, value, observed_at, basis)| {
                (
                    *input,
                    LiveValue {
                        value: *value,
                        observed_at: *observed_at,
                        basis: basis.clone(),
                    },
                )
            })
            .collect(),
    }
}

fn figure(value: f64, basis: Basis, mixed: bool) -> Option<Figure<f64>> {
    Some(Figure {
        value,
        basis,
        mixed,
    })
}

/// A lawn whose plan has air, humidity and rain, 10 cm soil temperature and moisture.
fn planned() -> Vec<Input> {
    vec![
        Input::AirTemp,
        Input::Humidity,
        Input::Rain,
        Input::SoilTemp10,
        Input::SoilMoisture10,
    ]
}

fn record(reading: LiveReading) -> RecordSummary {
    RecordSummary {
        live: reading,
        soil_temp_7day_avg_f: figure(47.5, station(), false),
        ambient_temp_7day_avg_f: figure(44.0, sensor(), true),
        humidity_7day_avg: figure(61.0, sensor(), false),
        precipitation_7day_total_mm: figure(12.5, analysis(), false),
        soil_temp_trend: Trend::Falling,
    }
}

fn everything_current() -> LiveReading {
    live(&[
        (Input::AirTemp, 43.5, at(17), sensor()),
        (Input::Humidity, 53.4, at(17), sensor()),
        (Input::Rain, 0.0, at(17), analysis()),
        (Input::SoilTemp10, 46.8, at(16), station()),
    ])
}

fn gathered(reading: LiveReading) -> Gathered {
    Gathered {
        record: record(reading),
        planned: planned(),
        soil_last_seen: Some(at(16)),
        gdd: Some(3240.5),
        forecast: None,
    }
}

fn standing(summary: &EnvironmentalSummary, input: &str) -> BasisStanding {
    summary
        .current_bases
        .iter()
        .find(|b| b.input == input)
        .unwrap_or_else(|| panic!("{input} has no entry"))
        .standing
}

// @spec SYNC-MERGE-007, SYNC-MERGE-003
#[test]
fn the_record_s_values_fill_the_summary_where_the_rules_read_them() {
    let summary = assemble(gathered(everything_current()), now());
    let current = summary.current.as_ref().unwrap();
    assert_eq!(current.ambient_temp_f, Some(43.5));
    assert_eq!(current.humidity_percent, Some(53.4));
    assert_eq!(current.precipitation_mm, Some(0.0));
    assert_eq!(current.soil_temp_10_f, Some(46.8));
    assert_eq!(current.soil_moisture_10, None, "planned but not current");
    assert_eq!(summary.soil_temp_7day_avg_f, Some(47.5));
    assert_eq!(summary.ambient_temp_7day_avg_f, Some(44.0));
    assert_eq!(summary.humidity_7day_avg, Some(61.0));
    assert_eq!(summary.precipitation_7day_total_mm, Some(12.5));
    assert_eq!(summary.soil_temp_trend, Trend::Falling);
    assert_eq!(summary.gdd_base50_ytd, Some(3240.5));
    assert_eq!(
        summary.last_updated,
        Some(now()),
        "assembled now, not observed now"
    );
}

// @spec SYNC-MERGE-010, WX-VIEW-API-007
#[test]
fn every_input_says_what_it_stands_on() {
    let summary = assemble(gathered(everything_current()), now());
    assert_eq!(summary.current_bases.len(), 13, "one per live input");
    let soil = summary
        .current_bases
        .iter()
        .find(|b| b.input == "soil_temp_10")
        .unwrap();
    assert_eq!(soil.standing, BasisStanding::InUse);
    assert_eq!(soil.basis, Some(station()));
    assert_eq!(soil.observed_at, Some(at(16)));
    assert_eq!(standing(&summary, "air_temp"), BasisStanding::InUse);
    assert_eq!(
        standing(&summary, "soil_moisture_10"),
        BasisStanding::NoCurrentValue
    );
    assert_eq!(standing(&summary, "soil_temp_50"), BasisStanding::NoSource);
    let moisture = summary
        .current_bases
        .iter()
        .find(|b| b.input == "soil_moisture_10")
        .unwrap();
    assert_eq!(
        (moisture.basis.as_ref(), moisture.observed_at),
        (None, None)
    );
}

// @spec SYNC-MERGE-010, WX-VIEW-API-007
#[test]
fn every_weekly_figure_says_what_it_stands_on() {
    let summary = assemble(gathered(everything_current()), now());
    let week = &summary.week_bases;
    assert_eq!(week["soil_temp_7day_avg_f"].basis, station());
    assert!(!week["soil_temp_7day_avg_f"].mixed);
    assert_eq!(week["ambient_temp_7day_avg_f"].basis, sensor());
    assert!(week["ambient_temp_7day_avg_f"].mixed);
    assert_eq!(week["precipitation_7day_total_mm"].basis, analysis());
    assert_eq!(week.len(), 4);
}

// @spec SYNC-MERGE-006, SYNC-MERGE-009
#[test]
fn soil_in_use_carries_its_own_stamp_and_is_not_stale() {
    let summary = assemble(gathered(everything_current()), now());
    assert!(!summary.soil_stale);
    assert_eq!(summary.soil_observed_at, Some(at(16)));
}

// @spec SYNC-MERGE-006, SYNC-MERGE-009, SYNC-AGE-003
#[test]
fn soil_withheld_for_age_is_stale_since_the_newest_value_held() {
    let silent_since = Utc.with_ymd_and_hms(2026, 11, 20, 0, 0, 0).unwrap();
    let reading = live(&[(Input::AirTemp, 43.5, at(17), sensor())]);
    let summary = assemble(
        Gathered {
            soil_last_seen: Some(silent_since),
            ..gathered(reading)
        },
        now(),
    );
    assert!(summary.soil_stale);
    assert_eq!(summary.soil_observed_at, Some(silent_since));
    assert_eq!(summary.current.as_ref().unwrap().soil_temp_10_f, None);
    assert_eq!(summary.soil_temp_7day_avg_f, None, "built on withheld soil");
    assert_eq!(summary.soil_temp_trend, Trend::Unknown);
    assert!(!summary.week_bases.contains_key("soil_temp_7day_avg_f"));
    // The other figures stand.
    assert_eq!(summary.ambient_temp_7day_avg_f, Some(44.0));
    assert_eq!(
        standing(&summary, "soil_temp_10"),
        BasisStanding::NoCurrentValue
    );
}

// @spec SYNC-AGE-003
#[test]
fn a_weekly_figure_built_on_a_withheld_input_is_left_empty() {
    // Air and humidity withheld: the sensor has been silent past its hour.
    let reading = live(&[(Input::SoilTemp10, 46.8, at(16), station())]);
    let summary = assemble(gathered(reading), now());
    assert_eq!(summary.ambient_temp_7day_avg_f, None);
    assert_eq!(summary.humidity_7day_avg, None);
    assert_eq!(summary.precipitation_7day_total_mm, None);
    assert_eq!(summary.soil_temp_7day_avg_f, Some(47.5));
    assert_eq!(summary.week_bases.len(), 1);
}

// @spec SYNC-MERGE-006, SYNC-MERGE-009
#[test]
fn with_no_soil_source_at_all_soil_is_neither_stale_nor_stamped() {
    let reading = live(&[(Input::AirTemp, 43.5, at(17), sensor())]);
    let summary = assemble(
        Gathered {
            planned: vec![Input::AirTemp],
            soil_last_seen: None,
            ..gathered(reading)
        },
        now(),
    );
    assert!(!summary.soil_stale);
    assert_eq!(summary.soil_observed_at, None);
    assert_eq!(standing(&summary, "soil_temp_10"), BasisStanding::NoSource);
}

// @spec SYNC-FAIL-006, SYNC-FAIL-007
#[test]
fn the_forecast_held_is_served_until_it_is_six_hours_old() {
    let forecast = |fetched_at| WeatherForecast {
        fetched_at,
        provider: ForecastProvider::OpenMeteo,
        hourly: Vec::new(),
        daily_summary: Vec::new(),
    };
    let with = |fetched_at| Gathered {
        forecast: Some(forecast(fetched_at)),
        ..gathered(everything_current())
    };
    let fresh = now() - Duration::hours(5) - Duration::minutes(59);
    assert!(assemble(with(fresh), now()).forecast.is_some());
    assert!(assemble(with(now() - FORECAST_MAX_AGE), now())
        .forecast
        .is_none());
}
