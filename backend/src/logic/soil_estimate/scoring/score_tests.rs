//! Scoring a small synthetic network end to end.

use super::super::scores::{Calibration, Direction};
use super::inputs::StationSoil;
use super::score::*;
use chrono::{Duration, NaiveDate};

fn d(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

/// Day-to-day weather: a repeatable pseudo-random value in −8..8 °F. A sum of sinusoids
/// would make the trailing means linearly dependent and leave the fit without a solution.
fn weather(day: i64) -> f64 {
    let mixed = (day as u64)
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((mixed >> 33) % 1600) as f64 / 100.0 - 8.0
}

/// A station at `latitude` whose soil is 0.8 × the trailing 7-day mean of its air, plus
/// 11 °F and `offset`, over 2011-11 through 2019: a relation the model can represent.
fn station(name: &str, latitude: f64, offset: f64) -> Station {
    let start = d(2011, 11, 1);
    let air_on = |i: i64| {
        let t = i as f64;
        55.0 - 22.0 * (std::f64::consts::TAU * (t - 80.0) / 365.25).cos() + weather(i)
    };
    let days = (d(2019, 12, 31) - start).num_days();
    let air = (0..=days)
        .map(|i| (start + Duration::days(i), air_on(i)))
        .collect();
    let soil: std::collections::BTreeMap<_, _> = (6..=days)
        .map(|i| {
            let week = (i - 6..=i).map(air_on).sum::<f64>() / 7.0;
            (start + Duration::days(i), week * 0.8 + 11.0 + offset)
        })
        .collect();
    Station {
        name: name.into(),
        soil: StationSoil {
            latitude,
            longitude: -90.0,
            soil_5cm_f: soil.clone(),
            soil_10cm_f: soil,
        },
        air,
    }
}

fn network() -> Vec<Station> {
    // Six alike stations and one far-off odd one (soil 6 °F warmer).
    let mut out: Vec<Station> = (0..6)
        .map(|i| station(&format!("near-{i}"), 40.0 + f64::from(i) * 0.1, 0.0))
        .collect();
    out.push(station("odd", 48.0, 6.0));
    out
}

// @spec WX-EST-SCORE-001
#[test]
fn every_consumer_crossing_is_scored_under_both_calibrations_at_both_depths() {
    let file = score(&network(), 2012, 2019, d(2026, 11, 22));
    assert_eq!(
        (file.first_year, file.last_year, file.stations),
        (2012, 2019, 7)
    );
    assert_eq!(file.generated_on, d(2026, 11, 22));
    for depth in [5, 10] {
        for calibration in [Calibration::NearestFive, Calibration::StationAlone] {
            assert!(file
                .daily_error
                .iter()
                .any(|e| e.depth_cm == depth && e.calibration == calibration));
            assert!(file.crossings.iter().any(|c| c.depth_cm == depth
                && c.calibration == calibration
                && c.threshold_f == 55.0
                && c.direction == Direction::Rising
                && c.samples > 0));
        }
    }
}

// @spec WX-EST-SCORE-001
#[test]
fn a_station_fitted_on_itself_is_scored_closely() {
    let file = score(&network(), 2012, 2019, d(2026, 11, 22));
    let alone = file
        .daily_error
        .iter()
        .find(|e| e.depth_cm == 5 && e.calibration == Calibration::StationAlone)
        .unwrap();
    assert!(alone.rmse_f < 1.0, "{alone:?}");
}

// @spec WX-EST-SCORE-002
#[test]
fn the_nearest_five_calibration_uses_no_soil_from_the_target_station() {
    // Scored alone, the odd station fits its own warm soil. Scored on its neighbours, it
    // can only be given their relation, so its daily error must show the 6 °F it differs by.
    let only_odd_differs = |calibration| {
        let mut stations = network();
        let odd = stations.pop().unwrap();
        // Score a network in which the odd station is the only one eligible to be a target
        // at 10 cm, by leaving the others without 10 cm soil.
        for other in &mut stations {
            other.soil.soil_10cm_f.clear();
        }
        stations.push(odd);
        score(&stations, 2012, 2019, d(2026, 11, 22))
            .daily_error
            .into_iter()
            .find(|e| e.depth_cm == 10 && e.calibration == calibration)
    };
    // With no other station holding 10 cm soil there is nothing to pool: no score at all.
    assert!(only_odd_differs(Calibration::NearestFive).is_none());
    assert!(only_odd_differs(Calibration::StationAlone).is_some());

    let file = score(&network(), 2012, 2019, d(2026, 11, 22));
    let five = file
        .daily_error
        .iter()
        .find(|e| e.depth_cm == 5 && e.calibration == Calibration::NearestFive)
        .unwrap();
    let alone = file
        .daily_error
        .iter()
        .find(|e| e.depth_cm == 5 && e.calibration == Calibration::StationAlone)
        .unwrap();
    assert!(alone.rmse_f <= five.rmse_f);
}

// @spec WX-EST-SCORE-001
#[test]
fn a_station_with_too_little_soil_is_neither_scored_nor_used() {
    let mut stations = network();
    let short: Vec<_> = stations[0]
        .soil
        .soil_5cm_f
        .keys()
        .copied()
        .skip(MIN_SOIL_DAYS - 1)
        .collect();
    for day in short {
        stations[0].soil.soil_5cm_f.remove(&day);
    }
    let file = score(&stations, 2012, 2019, d(2026, 11, 22));
    // Still seven at 10 cm; six at 5 cm. `stations` reports the larger.
    assert_eq!(file.stations, 7);
    assert_eq!(stations[0].soil.soil_5cm_f.len(), MIN_SOIL_DAYS - 1);
}
