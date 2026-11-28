//! USCRN parsers against lines recorded from the live products on 2026-11-22.

use super::parse::*;
use crate::models::weather_record::{Input, Network};
use chrono::{NaiveDate, TimeZone, Utc};

/// The header and seven rows of `stations.tsv`: Avondale; two foreign sites; an Alabama
/// regional station; an Alaska station; a closed station.
const STATIONS: &str = "WBAN\tCOUNTRY\tSTATE\tLOCATION\tVECTOR\tNAME\tLATITUDE\tLONGITUDE\tELEVATION\tSTATUS\tCOMMISSIONING\tCLOSING\tOPERATION\tPAIRING\tNETWORK\tSTATION_ID
03761\tUS\tPA\tAvondale\t2 N\tStroud Water Research Center\t39.85\t-75.78\t400\tCommissioned\t2006-09-29 00:00:00.0\t\tOperational\t\tUSCRN\t1123
37201\tRS\tSA\tTiksi\t4 SSE\tRoshydromet Observatory at Tiksi\t71.58\t128.91\t30\tNon-comissioned\t\t2018-11-06 00:00:00.0\tClosed\t\tUSCRN\t1789
64757\tCA\tON\tEgbert\t1 W\tEnvironment Canada CARE site\t44.23\t-79.78\t807\tExperimental\t\t\tOperational\t\tUSCRN\t1112
23801\tUS\tAL\tTroy\t2 W\tTroy\t31.79\t-86\t472\tExperimental\t\t\tOperational\t\tAlabama-USRCRN\t1552
23583\tUS\tAK\tAleknagik\t1 NNE\tCity of Aleknagik, Aleknagik Airport\t59.28\t-158.61\t80\tCommissioned\t2020-10-13 00:00:00.0\t\tOperational\t\tUSCRN\t1801
54851\tUS\tOH\tCoshocton\t8 NNE\tNorth Appalachian Experimental Watershed (CRN site)\t40.36\t-81.78\t1120\tCommissioned\t2007-01-26 00:00:00.0\t2016-09-19 00:00:00.0\tClosed\t\tUSCRN\t1125
";

// @spec WX-SRC-USCRN-001
#[test]
fn the_catalog_keeps_operating_stations_of_the_contiguous_united_states() {
    let stations = catalog(STATIONS);
    let ids: Vec<&str> = stations.iter().map(|s| s.station_id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["03761", "23801"],
        "foreign, Alaska and closed stations are left out"
    );
    let avondale = &stations[0];
    assert_eq!(avondale.network, Network::Uscrn);
    assert_eq!(avondale.name, "Avondale 2 N (Stroud Water Research Center)");
    assert_eq!(avondale.state, "PA");
    assert_eq!((avondale.latitude, avondale.longitude), (39.85, -75.78));
    assert_eq!(avondale.elevation_ft, Some(400.0));
    assert_eq!(avondale.first_year, Some(2006));
}

// @spec WX-SRC-USCRN-001
#[test]
fn a_stations_locator_is_the_stem_of_its_data_files() {
    let stations = catalog(STATIONS);
    assert_eq!(stations[0].locator.as_deref(), Some("PA_Avondale_2_N"));
    assert_eq!(stations[1].locator.as_deref(), Some("AL_Troy_2_W"));
}

// @spec WX-SRC-USCRN-001
#[test]
fn a_list_without_the_expected_columns_gives_no_stations() {
    assert!(catalog("").is_empty());
    assert!(catalog("ID\tNAME\n1\tSomewhere\n").is_empty());
}

/// Three real `hourly02` rows (PA Avondale 2 N, 2026): a full hour with rain; the 0000 row
/// that ends a UTC day; an hour whose air and soil sensors were out.
const HOURLY: &str = "\
03761 20260118 2200 20260118 1700  2.622  -75.79   39.86    -0.9    -0.5    -0.1    -0.9     0.8      6 0     18 0      0 0 C    -0.9 0    -0.8 0    -1.2 0    88 0   0.357   0.348   0.343   0.350   0.301     1.3     1.5     1.5     2.8     3.8
03761 20260119 0000 20260118 1900  2.622  -75.79   39.86    -2.3    -1.8    -0.9    -2.3     1.1      0 0      0 0      0 0 C    -1.9 0    -1.2 0    -2.5 0    89 3   0.358   0.348   0.343   0.349   0.302     1.3     1.5     1.5     2.8     3.8
03761 20260119 0600 20260119 0100  2.622  -75.79   39.86 -9999.0 -9999.0 -9999.0 -9999.0     0.0      0 0      0 0      0 0 C   -12.9 0   -12.2 0   -13.5 0    53 0 -99.000 -99.000 -99.000 -99.000 -99.000 -9999.0 -9999.0 -9999.0 -9999.0 -9999.0
a short line
";

// @spec WX-SRC-USCRN-002, WX-SRC-NORM-001
#[test]
fn an_hour_carries_air_humidity_rain_and_five_depths_in_record_units() {
    let rows = hourly02(HOURLY);
    assert_eq!(rows.len(), 3, "the short line is skipped");
    let row = &rows[0];
    let close = |got: Option<f64>, want: f64| (got.unwrap() - want).abs() < 1e-9;
    assert!(close(row.air_temp_f, 31.1)); // −0.5 °C, the hour's average
    assert!(close(row.air_temp_max_f, 31.82)); // −0.1 °C
    assert!(close(row.air_temp_min_f, 30.38)); // −0.9 °C
    assert_eq!((row.rh_pct, row.precip_mm), (Some(88.0), Some(0.8)));
    assert!(close(row.soil_temp_f[0], 34.34)); // 1.3 °C at 5 cm
    assert!(close(row.soil_temp_f[4], 38.84)); // 3.8 °C at 100 cm
    assert_eq!(
        row.soil_moisture,
        [
            Some(0.357),
            Some(0.348),
            Some(0.343),
            Some(0.350),
            Some(0.301)
        ]
    );
}

// @spec WX-SRC-USCRN-003, WX-SRC-NORM-004
#[test]
fn an_hour_is_stamped_at_its_start_one_hour_before_the_files_time() {
    let rows = hourly02(HOURLY);
    assert_eq!(
        rows[0].observed_at,
        Utc.with_ymd_and_hms(2026, 1, 18, 21, 0, 0).unwrap()
    );
    // The 0000 row is the last hour of the previous UTC day.
    assert_eq!(
        rows[1].observed_at,
        Utc.with_ymd_and_hms(2026, 1, 18, 23, 0, 0).unwrap()
    );
}

// @spec WX-SRC-USCRN-004, WX-SRC-NORM-003
#[test]
fn humidity_with_a_quality_flag_other_than_zero_is_empty() {
    let rows = hourly02(HOURLY);
    assert_eq!(rows[1].rh_pct, None, "flag 3");
    assert!(rows[1].air_temp_f.is_some(), "the rest of the row stands");
}

// @spec WX-SRC-NORM-002
#[test]
fn missing_markers_are_empty_and_the_rest_of_the_row_stands() {
    let row = &hourly02(HOURLY)[2];
    assert_eq!(
        (row.air_temp_f, row.air_temp_max_f, row.air_temp_min_f),
        (None, None, None)
    );
    assert_eq!(row.soil_temp_f, [None; 5]);
    assert_eq!(row.soil_moisture, [None; 5]);
    assert_eq!((row.rh_pct, row.precip_mm), (Some(53.0), Some(0.0)));
}

// @spec WX-SRC-NORM-002
#[test]
fn implausible_values_are_empty() {
    let hot = HOURLY
        .lines()
        .next()
        .unwrap()
        .replace("    -0.5    -0.1", "    95.0    -0.1");
    assert_eq!(hourly02(&hot)[0].air_temp_f, None, "95 °C is 203 °F");
    let wet = HOURLY
        .lines()
        .next()
        .unwrap()
        .replace("     0.8      6", "   400.0      6");
    assert_eq!(hourly02(&wet)[0].precip_mm, None, "400 mm in an hour");
}

/// Two real `daily01` rows (PA Avondale 2 N, 2020) and one with soil missing at 5 cm.
const DAILY: &str = "\
03761 20200101  2.622  -75.79   39.86     4.9    -4.5     0.2     1.4     0.0     5.77 C    10.9    -8.2    -0.1    87.5    40.0    64.8   0.369   0.319   0.354   0.356   0.314     3.6     3.8     4.1     4.4     5.0
03761 20200103  2.622  -75.79   39.86     9.9     4.9     7.4     7.8     6.7     1.80 C     9.2     4.3     6.9    98.5    85.2    93.7 -99.000 -99.000 -99.000 -99.000 -99.000 -9999.0     3.9 -9999.0 -9999.0 -9999.0
";

// @spec WX-SRC-USCRN-005, WX-SRC-NORM-004
#[test]
fn a_day_carries_air_rain_and_soil_at_5_and_10_cm_on_the_stations_own_day() {
    let rows = daily01(DAILY);
    let close = |got: Option<f64>, want: f64| (got.unwrap() - want).abs() < 1e-9;
    assert_eq!(rows[0].day, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap());
    assert!(close(rows[0].air_max_f, 40.82) && close(rows[0].air_min_f, 23.9));
    assert!(close(rows[0].air_mean_f, 32.36) && close(rows[0].soil_temp_5_f, 38.48));
    assert!(close(rows[0].soil_temp_10_f, 38.84));
    assert_eq!(rows[1].precip_mm, Some(6.7));
    assert_eq!(rows[1].soil_temp_5_f, None);
    assert!(close(rows[1].soil_temp_10_f, 39.02));
}

// @spec WX-SRC-USCRN-001
#[test]
fn what_a_station_reports_is_read_from_its_rows() {
    let reported = inputs_reported(&hourly02(HOURLY));
    for input in [
        Input::AirTemp,
        Input::Humidity,
        Input::Rain,
        Input::SoilTemp5,
        Input::SoilMoisture100,
    ] {
        assert!(reported.contains(&input), "{input:?}");
    }
    let dead = hourly02(HOURLY.lines().nth(2).unwrap());
    assert_eq!(inputs_reported(&dead), vec![Input::Humidity, Input::Rain]);
}

// @spec WX-SRC-ERR-001
#[test]
fn the_lines_a_file_could_not_read_are_counted_for_the_log() {
    let rows = hourly02(HOURLY);
    assert_eq!(unreadable_lines(HOURLY, rows.len()), 1, "the short line");
    let clean: String = HOURLY.lines().take(3).map(|l| format!("{l}\n\n")).collect();
    assert_eq!(
        unreadable_lines(&clean, hourly02(&clean).len()),
        0,
        "blank lines are not unreadable"
    );
}
