use super::inputs::*;
use chrono::NaiveDate;

fn d(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

/// Two real `daily01` rows (PA Avondale 2 N, 2020) and one with missing soil.
const DAILY01: &str = "\
03761 20200101  2.622  -75.79   39.86     4.9    -4.5     0.2     1.4     0.0     5.77 C    10.9    -8.2    -0.1    87.5    40.0    64.8   0.369   0.319   0.354   0.356   0.314     3.6     3.8     4.1     4.4     5.0
03761 20200102  2.622  -75.79   39.86     7.9    -5.9     1.0     1.4     0.0     5.87 C    13.8    -8.9    -0.2    89.7    49.8    73.5   0.360   0.312   0.349   0.353   0.310     2.0     2.4     3.2     4.2     5.1
03761 20200103  2.622  -75.79   39.86     9.9     4.9     7.4     7.8     6.7     1.80 C     9.2     4.3     6.9    98.5    85.2    93.7 -99.000 -99.000 -99.000 -99.000 -99.000 -9999.0     3.9 -9999.0 -9999.0 -9999.0
this line is not a record
";

// @spec WX-EST-SCORE-001
#[test]
fn daily01_gives_soil_at_5_and_10_cm_in_fahrenheit_and_the_stations_position() {
    let mut station = StationSoil::default();
    parse_daily01(DAILY01, &mut station);
    assert_eq!((station.latitude, station.longitude), (39.86, -75.79));
    let five = &station.soil_5cm_f;
    assert!((five[&d(2020, 1, 1)] - 38.48).abs() < 1e-9); // 3.6 °C
    assert!((five[&d(2020, 1, 2)] - 35.6).abs() < 1e-9); // 2.0 °C
    assert!((station.soil_10cm_f[&d(2020, 1, 1)] - 38.84).abs() < 1e-9); // 3.8 °C
}

// @spec WX-EST-SCORE-001
#[test]
fn daily01_skips_missing_markers_and_lines_that_are_not_records() {
    let mut station = StationSoil::default();
    parse_daily01(DAILY01, &mut station);
    assert!(
        !station.soil_5cm_f.contains_key(&d(2020, 1, 3)),
        "−9999 at 5 cm"
    );
    assert!((station.soil_10cm_f[&d(2020, 1, 3)] - 39.02).abs() < 1e-9); // 3.9 °C
    assert_eq!(station.soil_5cm_f.len(), 2);
    assert_eq!(station.soil_10cm_f.len(), 3);
}

// @spec WX-EST-SCORE-001
#[test]
fn a_longitude_west_of_90_degrees_is_a_position_not_a_missing_marker() {
    let mut station = StationSoil::default();
    parse_daily01(
        &DAILY01.replace("  -75.79   39.86", " -102.80   31.62"),
        &mut station,
    );
    assert_eq!((station.latitude, station.longitude), (31.62, -102.8));
}

// @spec WX-EST-SCORE-001
#[test]
fn several_files_accumulate_into_one_station() {
    let mut station = StationSoil::default();
    parse_daily01(DAILY01, &mut station);
    parse_daily01(&DAILY01.replace(" 2020010", " 2021010"), &mut station);
    assert_eq!(station.soil_5cm_f.len(), 4);
}

// @spec WX-EST-SCORE-001
#[test]
fn grid_air_is_half_the_sum_of_maximum_and_minimum_leaving_out_missing_days() {
    let json = r#"{"data":[["2011-11-01",52.6,32.9],["2011-11-02",-999,32.1],
                           ["2011-11-03",59.0,-999.0],["2011-11-04",60.0,40.0]]}"#;
    let air = parse_grid(json).unwrap();
    assert_eq!(air.len(), 2);
    assert!((air[&d(2011, 11, 1)] - 42.75).abs() < 1e-9);
    assert_eq!(air[&d(2011, 11, 4)], 50.0);
}

// @spec WX-EST-SCORE-001
#[test]
fn a_grid_body_that_is_not_the_expected_shape_is_an_error() {
    assert!(parse_grid(r#"{"error":"unknown grid"}"#).is_err());
    assert!(parse_grid("not json").is_err());
}
