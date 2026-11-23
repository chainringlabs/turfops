//! Choosing what a depth's model is fitted on.

use super::*;

fn station(network: Network, id: &str, miles_north: f64, first: i32, last: Option<i32>) -> Station {
    Station {
        network,
        station_id: id.into(),
        name: id.into(),
        state: "PA".into(),
        latitude: 40.0 + miles_north / 69.09,
        longitude: -76.0,
        elevation_ft: None,
        inputs: vec![Input::SoilTemp5, Input::SoilTemp10],
        first_year: Some(first),
        last_year: last,
        locator: None,
        utc_offset_hours: None,
    }
}

fn refs(stations: &[Calibrator]) -> Vec<i64> {
    stations.iter().map(|s| s.station_ref).collect()
}

/// Seven candidates at 10, 20, … 70 miles, with catalog rows 1 to 7.
fn seven() -> Vec<Calibrator> {
    (1..=7)
        .map(|n| Calibrator {
            station_ref: n,
            distance_miles: 10.0 * n as f64,
        })
        .collect()
}

fn days(known: &[(i64, usize)]) -> HashMap<i64, usize> {
    known.iter().copied().collect()
}

const BASIS: Calibrator = Calibrator {
    station_ref: 99,
    distance_miles: 12.0,
};

// @spec WX-EST-CAL-008
#[test]
fn candidates_are_uscrn_stations_with_the_depth_and_three_years_nearest_first() {
    let mut no_ten = station(Network::Uscrn, "shallow", 5.0, 2006, None);
    no_ten.inputs = vec![Input::SoilTemp5];
    let catalog: Vec<(i64, Station)> = (1..)
        .zip(vec![
            station(Network::Uscrn, "far", 80.0, 2006, None),
            station(Network::Scan, "scan", 10.0, 2000, None),
            station(Network::Uscrn, "new", 15.0, 2024, None),
            station(Network::Uscrn, "near", 20.0, 2023, None),
            station(Network::Uscrn, "closed", 30.0, 2006, Some(2010)),
            station(Network::Uscrn, "brief", 35.0, 2006, Some(2007)),
            no_ten,
        ])
        .collect();
    let got = candidates(40.0, -76.0, Input::SoilTemp10, 2026, &catalog);
    assert_eq!(refs(&got), vec![4, 5, 1]);
    assert!((got[0].distance_miles - 20.0).abs() < 0.05);
    let shallow = candidates(40.0, -76.0, Input::SoilTemp5, 2026, &catalog);
    assert_eq!(shallow[0].station_ref, 7);
}

// @spec WX-EST-CAL-002
#[test]
fn a_basis_station_with_a_thousand_days_is_fitted_on_alone() {
    let decision = decide(Some(BASIS), &seven(), &days(&[(99, 1000)]));
    assert_eq!(
        decision,
        Decision::Fit(Calibration::StationAlone, vec![BASIS])
    );
}

// @spec WX-EST-CAL-001
#[test]
fn a_basis_station_with_too_little_soil_history_gives_way_to_the_five_nearest() {
    let all: Vec<(i64, usize)> = (1..=7).map(|n| (n, 3000)).chain([(99, 999)]).collect();
    let decision = decide(Some(BASIS), &seven(), &days(&all));
    let Decision::Fit(Calibration::NearestFive, chosen) = decision else {
        panic!("{decision:?}");
    };
    assert_eq!(refs(&chosen), vec![1, 2, 3, 4, 5]);
}

// @spec WX-EST-FAIL-001
#[test]
fn nothing_is_fitted_while_a_chosen_stations_history_has_not_been_counted() {
    assert_eq!(
        decide(Some(BASIS), &seven(), &days(&[])),
        Decision::Wait(vec![BASIS])
    );
    let decision = decide(None, &seven(), &days(&[(1, 3000), (2, 3000)]));
    let Decision::Wait(wanted) = decision else {
        panic!("{decision:?}");
    };
    assert_eq!(refs(&wanted), vec![1, 2, 3, 4, 5]);
}

// @spec WX-EST-CAL-008
#[test]
fn a_chosen_station_that_proves_short_is_replaced_by_the_next_nearest() {
    let known = days(&[(1, 3000), (2, 400), (3, 3000), (4, 3000), (5, 3000)]);
    let Decision::Wait(wanted) = decide(None, &seven(), &known) else {
        panic!("the sixth has not been counted");
    };
    assert_eq!(refs(&wanted), vec![1, 3, 4, 5, 6]);
    let mut known = known;
    known.insert(6, 1500);
    let Decision::Fit(_, chosen) = decide(None, &seven(), &known) else {
        panic!("all five are counted");
    };
    assert_eq!(refs(&chosen), vec![1, 3, 4, 5, 6]);
}

// @spec WX-EST-FAIL-002
#[test]
fn three_or_four_qualifying_stations_are_fitted_on_and_fewer_are_not() {
    let known = days(&[(1, 3000), (2, 3000), (3, 3000), (4, 10)]);
    let four = &seven()[..4];
    let Decision::Fit(Calibration::NearestFive, chosen) = decide(None, four, &known) else {
        panic!("three qualify");
    };
    assert_eq!(chosen.len(), 3);
    let two = days(&[(1, 3000), (2, 3000), (3, 10), (4, 10)]);
    assert_eq!(decide(None, four, &two), Decision::NoModel);
    assert_eq!(decide(None, &[], &days(&[])), Decision::NoModel);
}
