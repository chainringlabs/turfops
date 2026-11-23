//! The anchored series around gaps in a measured one.

use super::*;

fn d(day: i64) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 3, 1).unwrap() + Duration::days(day - 1)
}

/// An estimate of 50 °F on every day from the 1st for `days` days.
fn flat(days: i64) -> Days {
    (1..=days).map(|day| (d(day), 50.0)).collect()
}

fn measured(days: impl IntoIterator<Item = i64>, value: f64) -> Days {
    days.into_iter().map(|day| (d(day), value)).collect()
}

// @spec WX-EST-ANCHOR-004
#[test]
fn days_before_the_first_measured_value_are_the_estimate_as_it_is() {
    let got = anchored(&flat(20), &measured(11..=20, 54.0));
    assert_eq!(got.len(), 10);
    assert!(got.values().all(|value| *value == 50.0));
}

// @spec WX-EST-ANCHOR-001, WX-EST-ANCHOR-002
#[test]
fn a_gap_with_measurements_on_both_sides_runs_from_one_edges_offset_to_the_others() {
    let mut series = measured(1..=10, 54.0); // +4 before the gap
    series.extend(measured(16..=25, 51.0)); // +1 after it
    let got = anchored(&flat(25), &series);
    let values: Vec<_> = (11..=15).map(|day| got[&d(day)]).collect();
    assert_eq!(values, vec![53.5, 53.0, 52.5, 52.0, 51.5]);
    assert_eq!(got.len(), 5, "measured days are not anchored");
}

// @spec WX-EST-ANCHOR-001
#[test]
fn an_edges_offset_is_the_mean_over_the_measured_days_among_the_seven_nearest() {
    // Before the gap: the 4th to the 10th are the seven nearest; only three are measured,
    // at +2, +4 and +6. The 1st, at +30, is too far to count.
    let mut series = Days::new();
    series.insert(d(1), 80.0);
    series.extend([(d(4), 52.0), (d(7), 54.0), (d(10), 56.0)]);
    let got = anchored(&flat(40), &series);
    // An open gap: the day after the 10th has lost a thirtieth of the +4.
    assert_eq!(got[&d(11)], 53.9);
}

// @spec WX-EST-ANCHOR-003
#[test]
fn an_open_gaps_offset_shrinks_to_nothing_over_thirty_days() {
    let got = anchored(&flat(60), &measured(1..=10, 56.0));
    assert_eq!(got[&d(11)], 55.8, "a day after the last measurement");
    assert_eq!(got[&d(25)], 53.0, "halfway");
    assert_eq!(got[&d(40)], 50.0, "thirty days on");
    assert_eq!(got[&d(60)], 50.0, "and the estimate alone thereafter");
}

// @spec WX-EST-ANCHOR-002
#[test]
fn an_edge_with_no_estimate_beside_it_takes_the_other_edges_offset() {
    let mut estimate = flat(25);
    for day in 16..=25 {
        estimate.remove(&d(day));
    }
    let mut series = measured(1..=10, 53.0);
    series.extend(measured(16..=25, 70.0));
    let got = anchored(&estimate, &series);
    assert!((11..=15).all(|day| got[&d(day)] == 53.0));
}

// @spec WX-EST-ANCHOR-002
#[test]
fn a_one_day_hole_sits_between_its_neighbours() {
    let mut series = measured(1..=10, 54.0);
    series.extend(measured(12..=20, 52.0));
    assert_eq!(anchored(&flat(20), &series)[&d(11)], 53.0);
}

/// A model that returns the day's air temperature: the one-day mean, nothing else.
fn passes_air_through() -> Model {
    Model {
        coefficients: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    }
}

// @spec WX-EST-FWD-001, WX-EST-FWD-002
#[test]
fn the_forward_series_continues_from_the_last_measured_day_and_loses_its_offset() {
    let air: DailyAir = (1..=50).map(|day| (d(day), 60.0)).collect();
    // Soil measured 3 °F under the model through the 40th; the record ends there.
    let got = forward(&passes_air_through(), &air, &measured(31..=40, 57.0), d(40));
    assert_eq!(*got.keys().next().unwrap(), d(41));
    assert_eq!(got.len(), 10);
    assert_eq!(got[&d(41)], 57.1);
    assert_eq!(got[&d(50)], 58.0, "a third of the way back to the model");
}

// @spec WX-EST-FWD-001
#[test]
fn a_forward_day_whose_trailing_means_are_not_usable_has_no_estimate() {
    // Twenty days of air: the 30-day mean needs 24.
    let air: DailyAir = (1..=20).map(|day| (d(day), 60.0)).collect();
    assert!(forward(&passes_air_through(), &air, &Days::new(), d(10)).is_empty());
    let enough: DailyAir = (1..=30).map(|day| (d(day), 60.0)).collect();
    let got = forward(&passes_air_through(), &enough, &Days::new(), d(10));
    assert_eq!(
        *got.keys().next().unwrap(),
        d(24),
        "the first day with 24 of its 30"
    );
    assert_eq!(got[&d(30)], 60.0, "with nothing measured, the model alone");
}
