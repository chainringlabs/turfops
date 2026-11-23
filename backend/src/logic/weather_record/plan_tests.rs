//! The basis plan for a lawn near Avondale, PA, among made-up stations.

use super::*;

pub(super) const LAWN: Lawn = Lawn {
    latitude: 39.86,
    longitude: -75.79,
    elevation_ft: Some(400.0),
};

pub(super) const SOIL: [Input; 4] = [
    Input::SoilTemp5,
    Input::SoilTemp10,
    Input::SoilMoisture5,
    Input::SoilMoisture10,
];
pub(super) const AIR: [Input; 2] = [Input::AirTemp, Input::Humidity];

/// A station `miles_north` of the lawn (a degree of latitude is 69.09 miles).
pub(super) fn station(
    network: Network,
    id: &str,
    miles_north: f64,
    feet: f64,
    inputs: &[Input],
) -> Station {
    Station {
        network,
        station_id: id.into(),
        name: format!("Station {id}"),
        state: "PA".into(),
        latitude: LAWN.latitude + miles_north / 69.09,
        longitude: LAWN.longitude,
        elevation_ft: Some(feet),
        inputs: inputs.to_vec(),
        first_year: Some(2006),
        last_year: None,
        locator: None,
        utc_offset_hours: None,
    }
}

pub(super) fn catalog(stations: Vec<Station>) -> Vec<(i64, Station)> {
    (1..).zip(stations).collect()
}

pub(super) fn all(inputs: &[&[Input]]) -> Vec<Input> {
    inputs.concat()
}

/// The source keys ranked for `input`, in order.
fn keys(plan: &[Ranked], input: Input) -> Vec<&str> {
    let ranked: Vec<_> = plan.iter().filter(|r| r.input == input).collect();
    let ranks: Vec<i32> = ranked.iter().map(|r| r.rank).collect();
    assert_eq!(ranks, (1..=ranked.len() as i32).collect::<Vec<_>>());
    ranked.iter().map(|r| r.source.key.as_str()).collect()
}

fn plan_among(stations: Vec<Station>) -> Vec<Ranked> {
    build(Some(&LAWN), &catalog(stations), &[], &[])
}

const ESTIMATE: &str = "estimate:39.8600,-75.7900";
const PRISM: &str = "grid:prism:39.8600,-75.7900";
const NRCC: &str = "grid:nrcc:39.8600,-75.7900";
const ANALYSIS: &str = "analysis:39.8600,-75.7900";

// @spec WX-REC-PLAN-011
#[test]
fn a_lawn_with_no_location_has_no_plan() {
    let stations = catalog(vec![station(Network::Uscrn, "1", 5.0, 400.0, &SOIL)]);
    assert!(build(None, &stations, &[], &[]).is_empty());
}

// @spec WX-REC-PLAN-004, WX-REC-PLAN-003
#[test]
fn soil_takes_the_nearest_uscrn_station_then_the_nearest_scan_station_then_the_estimate() {
    let plan = plan_among(vec![
        station(Network::Scan, "near", 8.0, 400.0, &SOIL),
        station(Network::Uscrn, "far", 40.0, 400.0, &SOIL),
        station(Network::Uscrn, "near", 20.0, 400.0, &SOIL),
        station(Network::Scan, "far", 30.0, 400.0, &SOIL),
    ]);
    assert_eq!(
        keys(&plan, Input::SoilTemp10),
        vec!["station:Uscrn:near", "station:Scan:near", ESTIMATE]
    );
    let first = &plan.iter().find(|r| r.input == Input::SoilTemp10).unwrap();
    assert_eq!(first.source.kind, SourceKind::Station);
    assert_eq!(first.source.name, "Station near, PA (USCRN)");
    assert!(!first.pinned);
}

// @spec WX-REC-PLAN-004
#[test]
fn a_soil_station_must_be_within_50_miles_and_report_the_input() {
    let plan = plan_among(vec![
        station(Network::Uscrn, "air-only", 5.0, 400.0, &AIR),
        station(Network::Uscrn, "beyond", 50.5, 400.0, &SOIL),
        station(Network::Scan, "edge", 49.5, 400.0, &[Input::SoilTemp10]),
    ]);
    assert_eq!(
        keys(&plan, Input::SoilTemp10),
        vec!["station:Scan:edge", ESTIMATE]
    );
    assert_eq!(keys(&plan, Input::SoilTemp5), vec![ESTIMATE]);
}

// @spec WX-REC-PLAN-010
#[test]
fn deep_soil_and_soil_moisture_have_no_estimate() {
    let plan = plan_among(vec![station(
        Network::Uscrn,
        "a",
        20.0,
        400.0,
        &[Input::SoilMoisture10],
    )]);
    assert_eq!(keys(&plan, Input::SoilMoisture10), vec!["station:Uscrn:a"]);
    for input in [Input::SoilMoisture5, Input::SoilTemp20, Input::SoilTemp100] {
        assert!(keys(&plan, input).is_empty(), "{input:?}");
    }
}

// @spec WX-REC-PLAN-005
#[test]
fn hourly_air_takes_a_station_within_10_miles_that_reports_both_then_the_analysis() {
    let plan = plan_among(vec![
        station(Network::Uscrn, "temp-only", 2.0, 400.0, &[Input::AirTemp]),
        station(Network::Uscrn, "both", 9.0, 400.0, &AIR),
        station(Network::Scan, "beyond", 11.0, 400.0, &AIR),
    ]);
    for input in AIR {
        assert_eq!(keys(&plan, input), vec!["station:Uscrn:both", ANALYSIS]);
    }
    let none_near = plan_among(vec![station(Network::Uscrn, "soil", 11.0, 400.0, &AIR)]);
    assert_eq!(keys(&none_near, Input::Humidity), vec![ANALYSIS]);
}

// @spec WX-REC-PLAN-006, WX-REC-PLAN-010
#[test]
fn daily_air_and_rain_come_from_the_grids_whatever_stations_are_near() {
    let everything = all(&[&SOIL, &AIR, &[Input::DailyAir, Input::Rain]]);
    let plan = plan_among(vec![station(Network::Uscrn, "a", 1.0, 400.0, &everything)]);
    assert_eq!(keys(&plan, Input::DailyAir), vec![PRISM, NRCC]);
    assert_eq!(keys(&plan, Input::Rain), vec![PRISM, ANALYSIS]);
}

// @spec WX-REC-PLAN-007
#[test]
fn a_station_more_than_1000_feet_above_or_below_the_lawn_is_passed_over() {
    let plan = plan_among(vec![
        station(Network::Uscrn, "ridge", 10.0, 1401.0, &SOIL),
        station(Network::Uscrn, "valley", 30.0, 1400.0, &SOIL),
        station(Network::Scan, "pit", 10.0, -601.0, &SOIL),
    ]);
    assert_eq!(
        keys(&plan, Input::SoilTemp5),
        vec!["station:Uscrn:valley", ESTIMATE]
    );
}

// @spec WX-REC-PLAN-013
#[test]
fn until_the_lawns_elevation_is_known_the_elevation_rule_is_not_applied() {
    let lawn = Lawn {
        elevation_ft: None,
        ..LAWN
    };
    let stations = catalog(vec![station(Network::Uscrn, "ridge", 10.0, 5000.0, &SOIL)]);
    let plan = build(Some(&lawn), &stations, &[], &[]);
    assert_eq!(
        keys(&plan, Input::SoilTemp5),
        vec!["station:Uscrn:ridge", ESTIMATE]
    );
}

// @spec WX-REC-PLAN-008, WX-REC-PLAN-009
#[test]
fn a_pinned_station_replaces_the_default_stations_at_any_distance_or_elevation() {
    let stations = catalog(vec![
        station(Network::Uscrn, "near", 10.0, 400.0, &SOIL),
        station(Network::Scan, "near", 12.0, 400.0, &SOIL),
        station(Network::Scan, "chosen", 120.0, 3000.0, &SOIL),
    ]);
    let pins = [
        Pin {
            input: Input::SoilTemp10,
            station_ref: 3,
        },
        Pin {
            input: Input::Rain,
            station_ref: 1,
        },
    ];
    let plan = build(Some(&LAWN), &stations, &[], &pins);
    assert_eq!(
        keys(&plan, Input::SoilTemp10),
        vec!["station:Scan:chosen", ESTIMATE]
    );
    let chosen = plan.iter().find(|r| r.input == Input::SoilTemp10).unwrap();
    assert!(chosen.pinned);
    assert!((chosen.source.distance_miles.unwrap() - 120.0).abs() < 0.1);
    assert_eq!(chosen.source.station_ref, Some(3));
    assert_eq!(
        keys(&plan, Input::Rain),
        vec!["station:Uscrn:near", PRISM, ANALYSIS],
        "the only way a station speaks for rain"
    );
    assert_eq!(
        keys(&plan, Input::SoilTemp5),
        vec!["station:Uscrn:near", "station:Scan:near", ESTIMATE],
        "other inputs keep their defaults"
    );
}

// @spec WX-REC-PLAN-008
#[test]
fn a_pin_on_a_station_the_catalog_no_longer_holds_falls_back_to_the_defaults() {
    let stations = catalog(vec![station(Network::Uscrn, "near", 10.0, 400.0, &SOIL)]);
    let pins = [Pin {
        input: Input::SoilTemp10,
        station_ref: 99,
    }];
    let plan = build(Some(&LAWN), &stations, &[], &pins);
    assert_eq!(
        keys(&plan, Input::SoilTemp10),
        vec!["station:Uscrn:near", ESTIMATE]
    );
}

// @spec WX-REC-PLAN-003
#[test]
fn the_owners_sensor_is_asked_first_and_an_air_sensor_also_speaks_for_daily_air() {
    let sensors = [
        MappedSensor {
            input: Input::AirTemp,
            key: "sensor:7".into(),
            name: "Patio".into(),
        },
        MappedSensor {
            input: Input::SoilTemp10,
            key: "sensor:9".into(),
            name: "Probe".into(),
        },
    ];
    let stations = catalog(vec![station(
        Network::Uscrn,
        "a",
        5.0,
        400.0,
        &all(&[&SOIL, &AIR]),
    )]);
    let plan = build(Some(&LAWN), &stations, &sensors, &[]);
    assert_eq!(
        keys(&plan, Input::AirTemp),
        vec!["sensor:7", "station:Uscrn:a", ANALYSIS]
    );
    assert_eq!(keys(&plan, Input::DailyAir), vec!["sensor:7", PRISM, NRCC]);
    assert_eq!(
        keys(&plan, Input::Humidity),
        vec!["station:Uscrn:a", ANALYSIS],
        "the sensor measures temperature only"
    );
    assert_eq!(
        keys(&plan, Input::SoilTemp10),
        vec!["sensor:9", "station:Uscrn:a", ESTIMATE]
    );
    let probe = plan.iter().find(|r| r.source.key == "sensor:9").unwrap();
    assert_eq!(
        (probe.source.kind, probe.source.name.as_str()),
        (SourceKind::Sensor, "Probe")
    );
}

// @spec WX-REC-PLAN-004, WX-REC-PLAN-005
#[test]
fn a_station_whose_inputs_are_not_yet_known_is_taken_to_report_everything() {
    let plan = plan_among(vec![station(Network::Uscrn, "new", 8.0, 400.0, &[])]);
    assert_eq!(
        keys(&plan, Input::SoilTemp10),
        vec!["station:Uscrn:new", ESTIMATE]
    );
    assert_eq!(
        keys(&plan, Input::Humidity),
        vec!["station:Uscrn:new", ANALYSIS]
    );
    assert_eq!(
        keys(&plan, Input::SoilMoisture50),
        vec!["station:Uscrn:new"]
    );
    assert_eq!(
        keys(&plan, Input::Rain),
        vec![PRISM, ANALYSIS],
        "still no station for rain"
    );
}
