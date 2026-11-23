//! The station catalog: every USCRN and SCAN station in the contiguous United States.

use crate::error::Result;
use crate::models::weather_record::{Input, Network, Station};
use sqlx::PgPool;

#[derive(sqlx::FromRow)]
struct StationRow {
    id: i64,
    network: String,
    station_id: String,
    name: String,
    state: String,
    latitude: f64,
    longitude: f64,
    elevation_ft: Option<f64>,
    inputs: Vec<String>,
    first_year: Option<i32>,
    last_year: Option<i32>,
    locator: Option<String>,
    utc_offset_hours: Option<f64>,
}

impl StationRow {
    /// `None` for a row whose network this build does not know.
    fn into_station(self) -> Option<(i64, Station)> {
        let station = Station {
            network: Network::parse(&self.network)?,
            station_id: self.station_id,
            name: self.name,
            state: self.state,
            latitude: self.latitude,
            longitude: self.longitude,
            elevation_ft: self.elevation_ft,
            inputs: self.inputs.iter().filter_map(|i| Input::parse(i)).collect(),
            first_year: self.first_year,
            last_year: self.last_year,
            locator: self.locator,
            utc_offset_hours: self.utc_offset_hours,
        };
        Some((self.id, station))
    }
}

/// Replaces one network's catalog with `stations`: new stations are added, known ones
/// updated in place (keeping their id), and stations the network no longer lists are
/// removed unless a source still refers to them.
// @spec WX-REC-PLAN-001
pub async fn replace_network(pool: &PgPool, network: Network, stations: &[Station]) -> Result<()> {
    let mut tx = pool.begin().await?;
    for station in stations.iter().filter(|s| s.network == network) {
        let inputs: Vec<&str> = station.inputs.iter().map(Input::as_str).collect();
        sqlx::query(
            r#"INSERT INTO weather_station
                   (network, station_id, name, state, latitude, longitude, elevation_ft, inputs,
                    first_year, last_year, locator, utc_offset_hours, refreshed_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW())
               ON CONFLICT (network, station_id) DO UPDATE SET
                   name = EXCLUDED.name, state = EXCLUDED.state, latitude = EXCLUDED.latitude,
                   longitude = EXCLUDED.longitude, elevation_ft = EXCLUDED.elevation_ft,
                   inputs = CASE WHEN cardinality(EXCLUDED.inputs) = 0 THEN weather_station.inputs
                                 ELSE EXCLUDED.inputs END, first_year = EXCLUDED.first_year,
                   last_year = EXCLUDED.last_year, locator = EXCLUDED.locator,
                   utc_offset_hours = EXCLUDED.utc_offset_hours, refreshed_at = NOW()"#,
        )
        .bind(network.as_str())
        .bind(&station.station_id)
        .bind(&station.name)
        .bind(&station.state)
        .bind(station.latitude)
        .bind(station.longitude)
        .bind(station.elevation_ft)
        .bind(&inputs)
        .bind(station.first_year)
        .bind(station.last_year)
        .bind(&station.locator)
        .bind(station.utc_offset_hours)
        .execute(&mut *tx)
        .await?;
    }
    let listed: Vec<&str> = stations.iter().map(|s| s.station_id.as_str()).collect();
    sqlx::query(
        r#"DELETE FROM weather_station
           WHERE network = $1 AND station_id <> ALL($2)
             AND id NOT IN (SELECT station_ref FROM weather_source WHERE station_ref IS NOT NULL)"#,
    )
    .bind(network.as_str())
    .bind(&listed)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Every station in the catalog with its row id, by network and station id.
// @spec WX-REC-PLAN-001
pub async fn stations(pool: &PgPool) -> Result<Vec<(i64, Station)>> {
    let rows = sqlx::query_as::<_, StationRow>(
        r#"SELECT id, network, station_id, name, state, latitude, longitude, elevation_ft, inputs,
                  first_year, last_year, locator, utc_offset_hours
           FROM weather_station ORDER BY network, station_id"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(StationRow::into_station)
        .collect())
}

/// When each network's catalog was last refreshed. A network with no stations is absent.
// @spec WX-REC-FILL-008
pub async fn refreshed(pool: &PgPool) -> Result<Vec<(Network, chrono::DateTime<chrono::Utc>)>> {
    let rows: Vec<(String, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as("SELECT network, max(refreshed_at) FROM weather_station GROUP BY network")
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(network, at)| Some((Network::parse(&network)?, at)))
        .collect())
}

/// Records which inputs a station's own data shows it reports. Returns whether that
/// changed what the catalog held.
pub async fn set_inputs(pool: &PgPool, station_ref: i64, inputs: &[Input]) -> Result<bool> {
    let names: Vec<&str> = inputs.iter().map(Input::as_str).collect();
    let changed = sqlx::query(
        "UPDATE weather_station SET inputs = $2 WHERE id = $1 AND inputs IS DISTINCT FROM $2",
    )
    .bind(station_ref)
    .bind(names)
    .execute(pool)
    .await?;
    Ok(changed.rows_affected() > 0)
}
