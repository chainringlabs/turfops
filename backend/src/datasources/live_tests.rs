//! One real request to each outside source, to confirm the parsers still match what the
//! services send. They need the network and are not part of the suite:
//! `cargo test live_ -- --ignored --test-threads=1`.

use super::grid::{Grid, GridClient};
use super::outside::Outside;
use super::scan::ScanClient;
use super::uscrn::{StationYear, UscrnClient};
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
