//! One fetch: a call to the source client for the product and span the record asked
//! for, and its rows and outcome written to the record together.

use crate::datasources::cadence::Product;
use crate::datasources::grid::{Grid, GridClient};
use crate::datasources::open_meteo::{OpenMeteoClient, Place};
use crate::datasources::outside::Outside;
use crate::datasources::scan::ScanClient;
use crate::datasources::uscrn::parse::inputs_reported;
use crate::datasources::uscrn::{StationYear, UscrnClient};
use crate::db::weather_queries::{catalog, fetch_state, lawn};
use crate::error::{Result, TurfOpsError};
use crate::logic::weather_record::due::{Fetch, Purpose};
use crate::models::weather_record::{HourlyRow, Network, Source, Station};
use chrono::{DateTime, Datelike, Duration, Utc};
use sqlx::PgPool;
use std::sync::Arc;

/// The clients of every outside weather source, sharing one request layer so that
/// requests to one host take turns.
pub struct Clients {
    pub uscrn: UscrnClient,
    pub scan: ScanClient,
    pub grid: GridClient,
    pub open_meteo: OpenMeteoClient,
}

impl Clients {
    pub fn new() -> Self {
        let outside = Arc::new(Outside::new());
        Self {
            uscrn: UscrnClient::new(outside.clone()),
            scan: ScanClient::new(outside.clone()),
            grid: GridClient::new(outside.clone()),
            open_meteo: OpenMeteoClient::new(outside),
        }
    }
}

impl Default for Clients {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Fetcher {
    pool: PgPool,
    clients: Clients,
}

/// The longest span the hourly analysis serves.
const ANALYSIS_MAX_DAYS: i64 = 92;

impl Fetcher {
    pub fn new(pool: PgPool, clients: Clients) -> Self {
        Self { pool, clients }
    }

    /// Runs one fetch and returns how many rows it wrote. Its rows and the span now held
    /// are written together; a failure is recorded against the source and product,
    /// logged, and returned.
    // @spec SYNC-SCHED-002, SYNC-SCHED-006, WX-REC-FILL-003, WX-REC-FAIL-004
    pub async fn run(&self, fetch: &Fetch, now: DateTime<Utc>) -> Result<usize> {
        let outcome = self.attempt(fetch, now).await;
        if let Err(error) = &outcome {
            tracing::warn!(source = %fetch.source.key, ?error, "weather fetch failed");
            let product = fetch.product.as_str();
            let said = error.owner_message();
            fetch_state::failed(&self.pool, fetch.source.id, product, &said, now).await?;
        }
        outcome
    }

    /// Candidate places for what the owner typed in setup.
    // @spec SETUP-API-001
    pub async fn places(&self, typed: &str) -> Result<Vec<Place>> {
        self.clients.open_meteo.places(typed).await
    }

    /// Reads a network's station list again and replaces the catalog's copy of it.
    // @spec WX-REC-FILL-008
    pub async fn refresh_catalog(&self, network: Network) -> Result<()> {
        let stations = match network {
            Network::Uscrn => self.clients.uscrn.catalog().await?,
            Network::Scan => self.clients.scan.catalog().await?,
        };
        catalog::replace_network(&self.pool, network, &stations).await
    }

    async fn attempt(&self, fetch: &Fetch, now: DateTime<Utc>) -> Result<usize> {
        match fetch.product {
            Product::StationHours => self.station_hours(fetch, now).await,
            Product::CalibrationDays => self.calibration_days(fetch, now).await,
            Product::PrismDays => self.grid_days(fetch, Grid::Prism, now).await,
            Product::NrccDays => self.grid_days(fetch, Grid::Nrcc, now).await,
            Product::AnalysisHours => self.analysis_hours(fetch, now).await,
            other => Err(TurfOpsError::Internal(format!(
                "{other:?} is not fetched into the weather record"
            ))),
        }
    }

    async fn station(&self, source: &Source) -> Result<Station> {
        let held = catalog::stations(&self.pool).await?;
        held.into_iter()
            .find(|(id, _)| Some(*id) == source.station_ref)
            .map(|(_, station)| station)
            .ok_or_else(|| {
                TurfOpsError::Internal(format!("{} is not in the station catalog", source.key))
            })
    }

    /// When the copy held was fetched, for a conditional request. A backfill has no
    /// copy to compare with.
    async fn held_since(&self, fetch: &Fetch) -> Result<Option<DateTime<Utc>>> {
        if fetch.purpose == Purpose::Backfill {
            return Ok(None);
        }
        let states = fetch_state::all(&self.pool).await?;
        Ok(states
            .into_iter()
            .find(|state| state.source_id == fetch.source.id)
            .and_then(|state| state.last_success))
    }

    async fn station_hours(&self, fetch: &Fetch, now: DateTime<Utc>) -> Result<usize> {
        let station = self.station(&fetch.source).await?;
        let (from, through) = fetch.span;
        let product = fetch.product.as_str();
        let mut rows = Vec::new();
        match station.network {
            Network::Scan => rows = self.clients.scan.hourly(&station, from, through).await?,
            Network::Uscrn => {
                let locator = locator(&station)?;
                let held_since = self.held_since(fetch).await?;
                let mut changed = false;
                for year in from.year()..=through.year() {
                    let answer = self.clients.uscrn.hourly(locator, year, held_since).await?;
                    if let StationYear::Rows {
                        rows: year_rows, ..
                    } = answer
                    {
                        changed = true;
                        rows.extend(year_rows);
                    }
                }
                if !changed {
                    fetch_state::checked(&self.pool, fetch.source.id, product, now).await?;
                    return Ok(0);
                }
                self.learn_inputs(fetch, &rows).await?;
                // A year's file holds the whole year; only the span asked for is new.
                let earliest = from - Duration::days(1);
                rows.retain(|row| row.observed_at.date_naive() >= earliest);
            }
        }
        fetch_state::hourly_fetched(&self.pool, fetch.source.id, product, &rows, fetch.span, now)
            .await?;
        Ok(rows.len())
    }

    /// The station list does not say what a USCRN station measures; the year so far
    /// of its own rows does. The plan is rebuilt from the catalog when this changes.
    // @spec WX-REC-PLAN-002
    async fn learn_inputs(&self, fetch: &Fetch, rows: &[HourlyRow]) -> Result<()> {
        let reported = inputs_reported(rows);
        let station_ref = fetch.source.station_ref;
        if let (Purpose::Current, Some(station_ref), false) =
            (fetch.purpose, station_ref, reported.is_empty())
        {
            catalog::set_inputs(&self.pool, station_ref, &reported).await?;
        }
        Ok(())
    }

    async fn calibration_days(&self, fetch: &Fetch, now: DateTime<Utc>) -> Result<usize> {
        let station = self.station(&fetch.source).await?;
        let locator = locator(&station)?;
        let (from, through) = fetch.span;
        let product = fetch.product.as_str();
        let held_since = self.held_since(fetch).await?;
        let mut rows = Vec::new();
        let mut changed = false;
        for year in from.year()..=through.year() {
            let answer = self.clients.uscrn.daily(locator, year, held_since).await?;
            if let StationYear::Rows {
                rows: year_rows, ..
            } = answer
            {
                changed = true;
                rows.extend(year_rows);
            }
        }
        if !changed {
            fetch_state::checked(&self.pool, fetch.source.id, product, now).await?;
            return Ok(0);
        }
        rows.retain(|row| (from..=through).contains(&row.day));
        fetch_state::daily_fetched(&self.pool, &fetch.source, product, &rows, fetch.span, now)
            .await?;
        Ok(rows.len())
    }

    async fn grid_days(&self, fetch: &Fetch, grid: Grid, now: DateTime<Utc>) -> Result<usize> {
        let (latitude, longitude) = point(&fetch.source)?;
        let (from, through) = fetch.span;
        let product = fetch.product.as_str();
        let days = self
            .clients
            .grid
            .fetch(grid, latitude, longitude, from, through)
            .await?
            .days;
        // The grid publishes a day behind: only what it returned is held.
        let span = match (days.last(), fetch.purpose) {
            (Some(newest), _) => (from, newest.day),
            // A past span the grid has nothing for is held as empty, or it would be
            // asked for again without end.
            (None, Purpose::Backfill) => fetch.span,
            (None, Purpose::Current) => {
                fetch_state::checked(&self.pool, fetch.source.id, product, now).await?;
                return Ok(0);
            }
        };
        fetch_state::daily_fetched(&self.pool, &fetch.source, product, &days, span, now).await?;
        Ok(days.len())
    }

    /// The hourly analysis, and with it the elevation of the lawn's point.
    // @spec WX-REC-PLAN-007
    async fn analysis_hours(&self, fetch: &Fetch, now: DateTime<Utc>) -> Result<usize> {
        let (latitude, longitude) = point(&fetch.source)?;
        let (from, through) = fetch.span;
        let days_back = ((through - from).num_days() + 1).clamp(1, ANALYSIS_MAX_DAYS) as u32;
        let analysis = self
            .clients
            .open_meteo
            .analysis(latitude, longitude, days_back, now)
            .await?;
        if let Some(feet) = analysis.elevation_ft {
            lawn::set_elevation_ft(&self.pool, feet).await?;
        }
        let product = fetch.product.as_str();
        let hours = &analysis.hours;
        fetch_state::hourly_fetched(&self.pool, fetch.source.id, product, hours, fetch.span, now)
            .await?;
        Ok(hours.len())
    }
}

fn locator(station: &Station) -> Result<&str> {
    station.locator.as_deref().ok_or_else(|| {
        TurfOpsError::Internal(format!(
            "station {} has no file locator",
            station.station_id
        ))
    })
}

fn point(source: &Source) -> Result<(f64, f64)> {
    source
        .latitude
        .zip(source.longitude)
        .ok_or_else(|| TurfOpsError::Internal(format!("{} has no point to fetch for", source.key)))
}
