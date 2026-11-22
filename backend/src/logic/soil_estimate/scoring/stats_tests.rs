use super::stats::*;

#[test]
fn median_is_the_middle_value_or_the_mean_of_the_two_middle_values() {
    assert_eq!(median(&[]), None);
    assert_eq!(median(&[5.0, 1.0, 3.0]), Some(3.0));
    assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
}

#[test]
fn percentile_is_nearest_rank_capped_at_the_last_value() {
    let values = [10.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
    assert_eq!(percentile(&values, 0.8), Some(9.0));
    assert_eq!(percentile(&values, 1.0), Some(10.0));
    assert_eq!(percentile(&[], 0.8), None);
}

// @spec WX-EST-SCORE-001
#[test]
fn a_summary_is_over_absolute_errors_with_the_signed_mean_as_bias() {
    let got = summarize(&[-4.0, 2.0, 6.0, -1.0, 0.0]).unwrap();
    assert_eq!(got.median, 2.0); // |errors| sorted: 0 1 2 4 6
    assert_eq!(got.p80, 6.0); // index floor(0.8 × 5) = 4
    assert!((got.bias - 0.6).abs() < 1e-12);
    assert_eq!(got.samples, 5);
    assert_eq!(summarize(&[]), None);
}

// @spec WX-EST-SCORE-001
#[test]
fn spread_is_each_years_distance_from_the_stations_median_date() {
    let dates = [100.0, 104.0, 96.0, 110.0, 101.0, 99.0]; // median 100.5
    let mut got = spread(&dates);
    got.sort_by(f64::total_cmp);
    assert_eq!(got, vec![0.5, 0.5, 1.5, 3.5, 4.5, 9.5]);
}

// @spec WX-EST-SCORE-001
#[test]
fn a_station_with_under_six_dated_years_contributes_no_spread() {
    assert!(spread(&[100.0, 104.0, 96.0, 110.0, 101.0]).is_empty());
    assert_eq!(MIN_YEARS_FOR_SPREAD, 6);
}
