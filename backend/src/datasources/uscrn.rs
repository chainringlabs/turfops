//! NOAA U.S. Climate Reference Network (WX-SRC-USCRN): the station list and the
//! `hourly02` and `daily01` station-year files.

pub mod parse;
#[cfg(test)]
mod parse_tests;
#[cfg(test)]
mod tests;

use super::outside::{Fetched, Outside};
use crate::error::{Result, Source, TurfOpsError};
use crate::models::weather_record::{DailyRow, HourlyRow, Network, Station};
use chrono::{DateTime, Utc};
use std::sync::Arc;

const BASE: &str = "https://www.ncei.noaa.gov/pub/data/uscrn/products";

/// One station-year file.
#[derive(Debug, Clone, PartialEq)]
pub enum StationYear<T> {
    /// The copy already held is current.
    Unchanged,
    /// The file's rows (none when the station has no file for the year) and when the
    /// network last changed it.
    Rows {
        rows: Vec<T>,
        last_modified: Option<DateTime<Utc>>,
    },
}

pub struct UscrnClient {
    outside: Arc<Outside>,
    base: String,
}

// @spec WX-SRC-CFG-004
// The station is an argument to every call; the client reads no setting.
impl UscrnClient {
    pub fn new(outside: Arc<Outside>) -> Self {
        Self {
            outside,
            base: BASE.into(),
        }
    }

    #[cfg(test)]
    pub fn with_base(mut self, base: &str) -> Self {
        self.base = base.into();
        self
    }

    /// The operating stations of the contiguous United States.
    // @spec WX-SRC-USCRN-001, WX-SRC-ERR-001
    pub async fn catalog(&self) -> Result<Vec<Station>> {
        let url = format!("{}/stations.tsv", self.base);
        let text = match self.outside.get(&url, None).await.map_err(down)? {
            Fetched::Body { text, .. } => text,
            other => return Err(down(format!("{url}: no station list ({other:?})"))),
        };
        let stations = parse::catalog(&text);
        if stations.is_empty() {
            return Err(down(format!(
                "{url}: no stations could be read from the list"
            )));
        }
        Ok(stations)
    }

    /// One station's hours for one year. `held_since` is when the network last changed
    /// the copy already held, if one is.
    // @spec WX-SRC-USCRN-002, WX-SRC-USCRN-006, WX-SRC-REQ-004
    pub async fn hourly(
        &self,
        locator: &str,
        year: i32,
        held_since: Option<DateTime<Utc>>,
    ) -> Result<StationYear<HourlyRow>> {
        let url = format!(
            "{}/hourly02/{year}/CRNH0203-{year}-{locator}.txt",
            self.base
        );
        self.station_year(&url, held_since, parse::hourly02).await
    }

    /// One station's days for one year.
    // @spec WX-SRC-USCRN-005, WX-SRC-USCRN-006, WX-SRC-REQ-004
    pub async fn daily(
        &self,
        locator: &str,
        year: i32,
        held_since: Option<DateTime<Utc>>,
    ) -> Result<StationYear<DailyRow>> {
        let url = format!("{}/daily01/{year}/CRND0103-{year}-{locator}.txt", self.base);
        self.station_year(&url, held_since, parse::daily01).await
    }

    // @spec WX-SRC-ERR-001
    async fn station_year<T>(
        &self,
        url: &str,
        held_since: Option<DateTime<Utc>>,
        parse: fn(&str) -> Vec<T>,
    ) -> Result<StationYear<T>> {
        match self.outside.get(url, held_since).await.map_err(down)? {
            Fetched::NotModified => Ok(StationYear::Unchanged),
            Fetched::Missing => Ok(StationYear::Rows {
                rows: Vec::new(),
                last_modified: None,
            }),
            Fetched::Body {
                text,
                last_modified,
            } => {
                let rows = parse(&text);
                if rows.is_empty() && !text.trim().is_empty() {
                    return Err(down(format!("{url}: the file could not be read")));
                }
                let skipped = parse::unreadable_lines(&text, rows.len());
                if skipped > 0 {
                    tracing::warn!(
                        url,
                        skipped,
                        "USCRN: lines that could not be read were skipped"
                    );
                }
                Ok(StationYear::Rows {
                    rows,
                    last_modified,
                })
            }
        }
    }
}

fn down(detail: String) -> TurfOpsError {
    TurfOpsError::unavailable(Source::SoilStation(Network::Uscrn), detail)
}
