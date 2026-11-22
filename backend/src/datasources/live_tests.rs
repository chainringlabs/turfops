//! One real request to each outside source, to confirm the parsers still match what the
//! services send. They need the network and are not part of the suite:
//! `cargo test live_ -- --ignored --test-threads=1`.

use super::grid::{Grid, GridClient};
use super::nws::NwsClient;
use super::open_meteo::OpenMeteoClient;
use super::outside::Outside;
use super::scan::ScanClient;
use super::uscrn::{StationYear, UscrnClient};
use crate::clock::Clock;
use chrono::{Datelike, Duration, Utc};
use std::sync::Arc;

#[tokio::test]
#[ignore = "calls NOAA"]
async fn live_uscrn_catalog_and_a_station_year() {
    let client = UscrnClient::new(Arc::new(Outside::new()));
    let stations = client.catalog().await.unwrap();
    assert!(stations.len() > 100, "{} stations", stations.len());
    let avondale = stations
        .iter()
        .find(|s| s.station_id == "03761")
        .expect("Avondale");
    let locator = avondale.locator.as_deref().unwrap();
    let year = Utc::now().year();
    let StationYear::Rows {
        rows,
        last_modified,
    } = client.hourly(locator, year, None).await.unwrap()
    else {
        panic!("expected rows");
    };
    assert!(
        rows.iter().any(|r| r.soil_temp_f[0].is_some()),
        "{} rows",
        rows.len()
    );
    let again = client.hourly(locator, year, last_modified).await.unwrap();
    assert_eq!(
        again,
        StationYear::Unchanged,
        "a second ask is answered not-modified"
    );
    let StationYear::Rows { rows, .. } = client.daily(locator, year - 1, None).await.unwrap()
    else {
        panic!("expected rows");
    };
    assert!(rows.len() > 300);
}

#[tokio::test]
#[ignore = "calls RCC-ACIS"]
async fn live_grid_days_for_a_point() {
    let client = GridClient::new(Arc::new(Outside::new()));
    let today = Utc::now().date_naive();
    for grid in [Grid::Prism, Grid::Nrcc] {
        let got = client
            .fetch(grid, 39.85, -75.78, today - Duration::days(10), today)
            .await
            .unwrap();
        assert!(got.days.len() >= 7, "{grid:?}: {} days", got.days.len());
        assert!((got.cell_latitude - 39.85).abs() < 0.1);
    }
}

#[tokio::test]
#[ignore = "calls USDA"]
async fn live_scan_catalog_and_hours() {
    let client = ScanClient::new(Arc::new(Outside::new()));
    let stations = client.catalog().await.unwrap();
    assert!(stations.len() > 100, "{} stations", stations.len());
    let station = stations
        .iter()
        .find(|s| s.station_id == "2028")
        .expect("Mahantango Ck");
    let through = Utc::now().date_naive() - Duration::days(3);
    let rows = client
        .hourly(station, through - Duration::days(1), through)
        .await
        .unwrap();
    assert!(
        rows.iter().any(|r| r.soil_temp_f[1].is_some()),
        "{} rows",
        rows.len()
    );
}

#[tokio::test]
#[ignore = "calls Open-Meteo"]
async fn live_open_meteo_analysis_and_place_search() {
    let client = OpenMeteoClient::new(Arc::new(Outside::new()));
    let now = Utc::now();
    let analysis = client.analysis(39.86, -75.79, 92, now).await.unwrap();
    let hours = &analysis.hours;
    assert!(hours.len() > 92 * 24 - 2, "{} hours", hours.len());
    let newest = hours.last().unwrap();
    assert!(now - newest.observed_at < Duration::hours(2));
    assert!(
        newest.air_temp_f.is_some() && newest.rh_pct.is_some() && newest.precip_mm.is_some(),
        "{newest:?}"
    );
    let elevation = analysis.elevation_ft.unwrap();
    assert!((300.0..600.0).contains(&elevation), "{elevation} ft");
    let places = client.places("19311").await.unwrap();
    assert_eq!(places[0].name, "Avondale");
    assert_eq!(places[0].state.as_deref(), Some("Pennsylvania"));
}

/// What a live forecast from either keyless provider must look like.
fn check_forecast(forecast: &crate::models::WeatherForecast, days: usize) {
    let now = Utc::now();
    let first = &forecast.hourly[0];
    assert!(first.timestamp <= now && now - first.timestamp < Duration::hours(1));
    assert!(forecast.hourly.iter().all(|p| p.slot_hours == 1));
    assert!(
        forecast
            .hourly
            .windows(2)
            .all(|w| w[1].timestamp > w[0].timestamp),
        "points are in order"
    );
    assert!(first.precipitation_mm.is_some() && first.precipitation_prob.is_some());
    let full = forecast.full_days().count();
    assert!(full >= days, "{full} full days");
}

#[tokio::test]
#[ignore = "calls Open-Meteo"]
async fn live_open_meteo_forecast() {
    let client = OpenMeteoClient::new(Arc::new(Outside::new()));
    let clock = Clock::Zone(chrono_tz::America::New_York);
    let forecast = client
        .forecast(39.86, -75.79, &clock, Utc::now())
        .await
        .unwrap();
    check_forecast(&forecast, 14);
    let last = forecast.full_days().last().unwrap();
    assert!(
        last.total_precipitation_mm.is_some(),
        "amounts run the whole span"
    );
}

#[tokio::test]
#[ignore = "calls the National Weather Service"]
async fn live_nws_forecast() {
    let client = NwsClient::new(Arc::new(Outside::new()));
    let clock = Clock::Zone(chrono_tz::America::New_York);
    let forecast = client
        .forecast(39.86, -75.79, &clock, Utc::now())
        .await
        .unwrap();
    check_forecast(&forecast, 5);
    let last = forecast.full_days().last().unwrap();
    assert_eq!(
        last.total_precipitation_mm, None,
        "amounts end after about three days"
    );
    assert!(last.max_precipitation_prob.is_some());
}
