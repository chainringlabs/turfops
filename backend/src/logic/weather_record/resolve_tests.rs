//! Which source speaks for a day.

use super::*;
use crate::models::weather_basis::BasisKind;
use crate::models::weather_record::{SourceKind, SourceRole};

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}

fn source(kind: SourceKind, name: &str) -> Source {
    Source {
        id: 0,
        key: name.into(),
        kind,
        name: name.into(),
        station_ref: None,
        latitude: None,
        longitude: None,
        distance_miles: Some(12.5),
        role: SourceRole::Basis,
    }
}

/// A source that covers `days` (its value is the day number) and has reached `reached`.
fn candidate(kind: SourceKind, name: &str, days: &[u32], reached: Option<u32>) -> Candidate<f64> {
    Candidate {
        source: source(kind, name),
        covered: days.iter().map(|day| (d(*day), f64::from(*day))).collect(),
        reached: reached.map(d),
    }
}

fn station(days: &[u32], reached: Option<u32>) -> Candidate<f64> {
    candidate(SourceKind::Station, "Avondale", days, reached)
}

fn estimate(days: &[u32]) -> Candidate<f64> {
    candidate(
        SourceKind::Estimate,
        "Soil estimate",
        days,
        days.last().copied(),
    )
}

/// Which candidate each day from the 1st through the 8th came from; `None` is no value.
fn speakers(candidates: &[Candidate<f64>], rule: Pending) -> Vec<Option<usize>> {
    let resolved = resolve(candidates, d(1), d(8), rule);
    (1..=8)
        .map(|day| resolved.get(&d(day)).map(|r| r.from))
        .collect()
}

const ALL: [u32; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

// @spec WX-REC-RES-001
#[test]
fn the_highest_ranked_source_that_covers_a_day_speaks_for_it() {
    let both = [station(&[1, 2, 3, 4, 5, 6, 7, 8], Some(8)), estimate(&ALL)];
    assert_eq!(speakers(&both, Pending::LeavesEmpty), vec![Some(0); 8]);
    let resolved = resolve(&both, d(1), d(8), Pending::LeavesEmpty);
    assert_eq!(resolved[&d(3)].value, 3.0);
}

// @spec WX-REC-RES-001
#[test]
fn a_hole_a_source_has_already_passed_is_filled_from_below() {
    let holed = [station(&[1, 2, 5, 6, 7, 8], Some(8)), estimate(&ALL)];
    assert_eq!(
        speakers(&holed, Pending::LeavesEmpty),
        vec![
            Some(0),
            Some(0),
            Some(1),
            Some(1),
            Some(0),
            Some(0),
            Some(0),
            Some(0)
        ]
    );
}

// @spec WX-REC-RES-010
#[test]
fn a_day_a_reporting_source_has_not_reached_is_left_empty() {
    // The station has part of the 6th and nothing later.
    let trailing = [station(&[1, 2, 3, 4, 5], Some(6)), estimate(&ALL)];
    assert_eq!(
        speakers(&trailing, Pending::LeavesEmpty),
        vec![
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            None,
            None,
            None
        ]
    );
}

// @spec WX-REC-RES-010
#[test]
fn a_source_that_has_gone_quiet_holds_nothing_back() {
    let quiet = [station(&[1, 2, 3], None), estimate(&ALL)];
    assert_eq!(
        speakers(&quiet, Pending::LeavesEmpty),
        vec![
            Some(0),
            Some(0),
            Some(0),
            Some(1),
            Some(1),
            Some(1),
            Some(1),
            Some(1)
        ]
    );
    let never = [station(&[], None), estimate(&ALL)];
    assert_eq!(speakers(&never, Pending::LeavesEmpty), vec![Some(1); 8]);
}

// @spec WX-REC-RES-010
#[test]
fn a_pending_source_also_holds_back_everything_ranked_below_it() {
    let three = [
        candidate(SourceKind::Sensor, "Probe", &[7, 8], Some(8)),
        station(&[1, 2, 3, 4], Some(5)),
        estimate(&ALL),
    ];
    assert_eq!(
        speakers(&three, Pending::LeavesEmpty),
        vec![
            Some(1),
            Some(1),
            Some(1),
            Some(1),
            None,
            None,
            Some(0),
            Some(0)
        ]
    );
}

// @spec WX-REC-RES-008
#[test]
fn rain_is_filled_from_below_while_the_grid_has_not_published() {
    assert_eq!(pending_rule(Input::Rain), Pending::FilledFromBelow);
    assert_eq!(pending_rule(Input::SoilTemp10), Pending::LeavesEmpty);
    assert_eq!(pending_rule(Input::DailyAir), Pending::LeavesEmpty);
    let grid = candidate(SourceKind::Grid, "PRISM", &[1, 2, 3, 4, 5, 6], Some(6));
    let analysis = candidate(SourceKind::Analysis, "Analysis", &ALL, Some(8));
    assert_eq!(
        speakers(&[grid, analysis], Pending::FilledFromBelow),
        vec![
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
            Some(1),
            Some(1)
        ]
    );
}

// @spec WX-REC-FAIL-005
#[test]
fn a_day_no_source_covers_has_no_value() {
    let gap = [
        station(&[1, 2, 7, 8], Some(8)),
        estimate(&[1, 2, 3, 6, 7, 8]),
    ];
    let resolved = resolve(&gap, d(1), d(8), Pending::LeavesEmpty);
    assert!(!resolved.contains_key(&d(4)) && !resolved.contains_key(&d(5)));
    assert_eq!(resolved.len(), 6);
}

// @spec WX-REC-BASIS-003, WX-REC-BASIS-001, WX-REC-BASIS-002
#[test]
fn a_series_names_its_basis_as_spans_of_neighbouring_days() {
    let candidates = [
        station(&[1, 2, 5, 6, 7, 8], Some(8)),
        estimate(&[1, 2, 3, 8]),
    ];
    let resolved = resolve(&candidates, d(1), d(8), Pending::LeavesEmpty);
    let spans = spans(Input::SoilTemp10, &resolved, &candidates);
    let seen: Vec<_> = spans
        .iter()
        .map(|s| (s.from, s.to, s.basis.kind, s.basis.source.as_str()))
        .collect();
    assert_eq!(
        seen,
        vec![
            (d(1), d(2), BasisKind::Station, "Avondale"),
            (d(3), d(3), BasisKind::Estimate, "Soil estimate"),
            (d(5), d(8), BasisKind::Station, "Avondale"),
        ]
    );
    assert!(spans.iter().all(|s| s.input == Input::SoilTemp10));
    assert_eq!(spans[0].basis.distance_miles, Some(12.5));
    assert_eq!(
        spans[1].basis.distance_miles, None,
        "only a station has a distance"
    );
    assert!(!spans[1].basis.approximate);
}

// @spec WX-REC-BASIS-002
#[test]
fn the_hourly_analysis_is_an_approximate_estimate_and_a_grid_is_not() {
    let basis = |kind| Basis::from(&source(kind, "x"));
    let analysis = basis(SourceKind::Analysis);
    assert_eq!(
        (analysis.kind, analysis.approximate),
        (BasisKind::Estimate, true)
    );
    let grid = basis(SourceKind::Grid);
    assert_eq!((grid.kind, grid.approximate), (BasisKind::Estimate, false));
    assert_eq!(basis(SourceKind::Sensor).kind, BasisKind::Sensor);
}

fn hours(day: u32, hours: i64, mean: f64) -> HourlyDay {
    HourlyDay {
        day: d(day),
        air: Stat {
            hours,
            mean: Some(mean),
        },
        air_max: Some(mean + 10.0),
        air_min: Some(mean - 10.0),
        ..Default::default()
    }
}

// @spec WX-REC-RES-002
#[test]
fn an_hourly_source_covers_a_day_with_twelve_valid_hours() {
    let days = [hours(1, 24, 50.0), hours(2, 12, 51.0), hours(3, 11, 52.0)];
    let covered = covered_by_hours(&days, |day| mean(&day.air));
    assert_eq!(covered, BTreeMap::from([(d(1), 50.0), (d(2), 51.0)]));
}

// @spec WX-REC-API-007
#[test]
fn a_days_air_is_the_mean_of_its_hours_or_the_midpoint_of_a_grids_extremes() {
    let from_hours = covered_by_hours(&[hours(1, 24, 48.0)], air_of_hours);
    assert_eq!(
        from_hours[&d(1)],
        DayAir {
            max_f: 58.0,
            min_f: 38.0,
            mean_f: 48.0
        }
    );
    let row = DailyRow {
        day: d(1),
        air_max_f: Some(60.0),
        air_min_f: Some(40.0),
        air_mean_f: Some(47.0),
        ..Default::default()
    };
    assert_eq!(air_of_grid(&row).unwrap().mean_f, 50.0);
    let half = DailyRow {
        air_min_f: None,
        ..row
    };
    assert_eq!(air_of_grid(&half), None);
}

// @spec WX-REC-RES-007
#[test]
fn a_grids_rain_belongs_to_the_lawns_previous_day_and_its_air_to_the_same_day() {
    let rows = [DailyRow {
        day: d(5),
        air_max_f: Some(60.0),
        air_min_f: Some(40.0),
        precip_mm: Some(6.0),
        soil_temp_10_f: Some(51.0),
        ..Default::default()
    }];
    let rain = covered_by_days(&rows, GRID_RAIN_SHIFT_DAYS, |row| row.precip_mm);
    assert_eq!(rain, BTreeMap::from([(d(4), 6.0)]));
    let air = covered_by_days(&rows, 0, air_of_grid);
    assert!(air.contains_key(&d(5)));
    let soil = covered_by_days(&rows, 0, |row| row.soil_temp_10_f);
    assert_eq!(soil, BTreeMap::from([(d(5), 51.0)]));
    let none = covered_by_days(&rows, 0, |row| row.soil_temp_5_f);
    assert!(none.is_empty());
}
