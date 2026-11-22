//! The SCAN client against responses recorded from the AWDB REST API on 2026-11-22.

use super::*;
use crate::models::weather_record::{Input, Network};
use crate::test_support::{stub_http, stub_http_seeing};
use chrono::{TimeZone, Utc};

/// Mahantango Ck, trimmed to a handful of its 216 elements, and a station in Alaska.
const STATIONS: &str = r#"[
 {"stationTriplet":"2028:PA:SCAN","stationId":"2028","stateCode":"PA","networkCode":"SCAN",
  "name":"Mahantango Ck","dcoCode":"GC","countyName":"Northumberland","elevation":720.0,
  "latitude":40.67309,"longitude":-76.66702,"dataTimeZone":-5.0,
  "beginDate":"1999-04-27 13:00","endDate":"2100-01-01 00:00",
  "stationElements":[
   {"elementCode":"TOBS","durationName":"HOURLY","storedUnitCode":"degF"},
   {"elementCode":"RHUM","durationName":"HOURLY","storedUnitCode":"pct"},
   {"elementCode":"PRCP","durationName":"HOURLY","storedUnitCode":"in"},
   {"elementCode":"PRCP","durationName":"DAILY","storedUnitCode":"in"},
   {"elementCode":"STO","heightDepth":-2,"durationName":"HOURLY","storedUnitCode":"degF"},
   {"elementCode":"STO","heightDepth":-4,"durationName":"HOURLY","storedUnitCode":"degF"},
   {"elementCode":"STO","heightDepth":-40,"durationName":"DAILY","storedUnitCode":"degF"},
   {"elementCode":"SMS","heightDepth":-4,"durationName":"HOURLY","storedUnitCode":"pct"},
   {"elementCode":"WSPDV","durationName":"HOURLY","storedUnitCode":"mph"}]},
 {"stationTriplet":"2065:AK:SCAN","stationId":"2065","stateCode":"AK","networkCode":"SCAN",
  "name":"Aniak","elevation":80.0,"latitude":61.58,"longitude":-159.58,"dataTimeZone":-9.0,
  "beginDate":"2003-08-01 00:00","endDate":"2019-06-01 00:00","stationElements":[]}
]"#;

/// Two hours of Mahantango Ck: air and humidity, rain, soil at 2 and 4 in, moisture at 4 in.
const DATA: &str = r#"[{"stationTriplet":"2028:PA:SCAN","data":[
 {"stationElement":{"elementCode":"PRCP","durationName":"HOURLY","storedUnitCode":"in"},
  "values":[{"date":"2026-11-18 00:00","value":0.00},{"date":"2026-11-18 01:00","value":0.10}]},
 {"stationElement":{"elementCode":"RHUM","durationName":"HOURLY","storedUnitCode":"pct"},
  "values":[{"date":"2026-11-18 00:00","value":96.00},{"date":"2026-11-18 01:00","value":97.00}]},
 {"stationElement":{"elementCode":"SMS","heightDepth":-4,"durationName":"HOURLY","storedUnitCode":"pct"},
  "values":[{"date":"2026-11-18 00:00","value":28.5},{"date":"2026-11-18 01:00","value":27.0}]},
 {"stationElement":{"elementCode":"STO","heightDepth":-2,"durationName":"HOURLY","storedUnitCode":"degF"},
  "values":[{"date":"2026-11-18 00:00","value":44},{"date":"2026-11-18 01:00"}]},
 {"stationElement":{"elementCode":"STO","heightDepth":-4,"durationName":"HOURLY","storedUnitCode":"degF"},
  "values":[{"date":"2026-11-18 00:00","value":45},{"date":"2026-11-18 01:00","value":45}]},
 {"stationElement":{"elementCode":"TOBS","durationName":"HOURLY","storedUnitCode":"degF"},
  "values":[{"date":"2026-11-18 00:00","value":30.2},{"date":"2026-11-18 01:00","value":-99.9}]}
]}]"#;

fn client(base: &str) -> ScanClient {
    ScanClient::new(Arc::new(Outside::unpaced())).with_base(base)
}

// @spec WX-SRC-SCAN-001
#[test]
fn the_catalog_keeps_contiguous_stations_with_the_hourly_inputs_they_report() {
    let stations = parse::catalog(STATIONS).unwrap();
    assert_eq!(stations.len(), 1, "Alaska is left out");
    let station = &stations[0];
    assert_eq!(
        (station.network, station.station_id.as_str()),
        (Network::Scan, "2028")
    );
    assert_eq!(
        (station.name.as_str(), station.state.as_str()),
        ("Mahantango Ck", "PA")
    );
    assert_eq!((station.latitude, station.longitude), (40.67309, -76.66702));
    assert_eq!(
        (station.elevation_ft, station.utc_offset_hours),
        (Some(720.0), Some(-5.0))
    );
    assert_eq!((station.first_year, station.last_year), (Some(1999), None));
    assert_eq!(station.locator.as_deref(), Some("2028:PA:SCAN"));
    assert_eq!(
        station.inputs,
        vec![
            Input::AirTemp,
            Input::Humidity,
            Input::Rain,
            Input::SoilTemp5,
            Input::SoilTemp10,
            Input::SoilMoisture10
        ],
        "daily-only elements and elements the record has no input for are not listed"
    );
}

// @spec WX-SRC-SCAN-002, WX-SRC-NORM-001
#[test]
fn an_hour_carries_each_element_in_record_units_at_the_matching_depth() {
    let rows = parse::hourly(DATA, -5.0).unwrap();
    assert_eq!(rows.len(), 2);
    let first = &rows[0];
    assert_eq!(
        (first.air_temp_f, first.rh_pct, first.precip_mm),
        (Some(30.2), Some(96.0), Some(0.0))
    );
    assert_eq!(
        first.soil_temp_f,
        [Some(44.0), Some(45.0), None, None, None]
    );
    assert_eq!(first.soil_moisture, [None, Some(0.285), None, None, None]);
    assert!((rows[1].precip_mm.unwrap() - 2.54).abs() < 1e-9, "0.10 in");
}

// @spec WX-SRC-SCAN-003, WX-SRC-NORM-004
#[test]
fn times_are_converted_from_the_stations_standard_time_to_utc() {
    let rows = parse::hourly(DATA, -5.0).unwrap();
    assert_eq!(
        rows[0].observed_at,
        Utc.with_ymd_and_hms(2026, 11, 18, 5, 0, 0).unwrap()
    );
    let mountain = parse::hourly(DATA, -7.0).unwrap();
    assert_eq!(
        mountain[0].observed_at,
        Utc.with_ymd_and_hms(2026, 11, 18, 7, 0, 0).unwrap()
    );
}

// @spec WX-SRC-NORM-002
#[test]
fn a_value_that_is_absent_or_implausible_is_empty_and_the_rest_of_the_hour_stands() {
    let second = &parse::hourly(DATA, -5.0).unwrap()[1];
    assert_eq!(second.soil_temp_f[0], None, "no value given");
    assert_eq!(second.air_temp_f, None, "−99.9 °F");
    assert_eq!(
        (second.rh_pct, second.soil_temp_f[1]),
        (Some(97.0), Some(45.0))
    );
}

// @spec WX-SRC-NORM-001
#[test]
fn a_temperature_declared_in_celsius_is_converted() {
    let celsius = DATA.replace(
        r#""elementCode":"TOBS","durationName":"HOURLY","storedUnitCode":"degF""#,
        r#""elementCode":"TOBS","durationName":"HOURLY","storedUnitCode":"degC""#,
    );
    let rows = parse::hourly(&celsius, -5.0).unwrap();
    assert!(
        (rows[0].air_temp_f.unwrap() - 86.36).abs() < 1e-9,
        "30.2 °C"
    );
}

// @spec WX-SRC-SCAN-002
#[tokio::test]
async fn hours_are_asked_for_by_triplet_element_and_span() {
    let (base, request) = stub_http_seeing(200, DATA, "").await;
    let station = parse::catalog(STATIONS).unwrap().remove(0);
    let from = chrono::NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
    let rows = client(&base).hourly(&station, from, from).await.unwrap();
    assert_eq!(rows.len(), 2);
    let request = request.await.unwrap();
    let line = request.lines().next().unwrap();
    for part in [
        "GET /data?",
        "stationTriplets=2028:PA:SCAN",
        "duration=HOURLY",
        "STO:-2",
        "SMS:-40",
        "beginDate=2026-11-18%2000:00",
        "endDate=2026-11-18%2023:00",
    ] {
        assert!(line.contains(part), "{part} in {line}");
    }
}

// @spec WX-SRC-SCAN-002
#[tokio::test]
async fn more_than_a_year_at_once_is_refused_before_any_request() {
    let station = parse::catalog(STATIONS).unwrap().remove(0);
    let from = chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let through = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    // No server is listening at this address; a request would fail differently.
    let err = client("http://127.0.0.1:9")
        .hourly(&station, from, through)
        .await
        .unwrap_err();
    assert!(matches!(err, TurfOpsError::Internal(_)), "{err:?}");
}

// @spec WX-SRC-SCAN-001, WX-SRC-ERR-001
#[tokio::test]
async fn the_catalog_is_fetched_for_the_whole_network_and_a_bad_answer_is_an_outage() {
    let (base, request) = stub_http_seeing(200, STATIONS, "").await;
    assert_eq!(client(&base).catalog().await.unwrap().len(), 1);
    let request = request.await.unwrap();
    assert!(
        request.starts_with(
            "GET /stations?stationTriplets=*:*:SCAN&returnStationElements=true\
             &elements=TOBS,RHUM,PRCP,STO:*,SMS:*&durations=HOURLY "
        ),
        "{request}"
    );
    for (status, body) in [(500, "error"), (200, "not json"), (200, "[]")] {
        let base = stub_http(status, body).await;
        let err = client(&base).catalog().await.unwrap_err();
        assert!(err.owner_message().contains("station network"), "{err:?}");
    }
}
