//! USDA SCAN through the AWDB REST API (WX-SRC-SCAN): the station list and hourly data.

pub mod parse;
#[cfg(test)]
mod tests;

use super::outside::{Fetched, Outside};
use crate::error::{Result, Source, TurfOpsError};
use crate::models::weather_record::{HourlyRow, Station};
use chrono::NaiveDate;
use std::sync::Arc;

const BASE: &str = "https://wcc.sc.egov.usda.gov/awdbRestApi/services/v1";
/// Air temperature, humidity, precipitation, then soil temperature and soil moisture at
/// SCAN's five standard depths.
const ELEMENTS: &str = "TOBS,RHUM,PRCP,STO:-2,STO:-4,STO:-8,STO:-20,STO:-40,\
                        SMS:-2,SMS:-4,SMS:-8,SMS:-20,SMS:-40";
/// The longest span asked for in one request.
pub const MAX_SPAN_DAYS: i64 = 366;

pub struct ScanClient {
    outside: Arc<Outside>,
    base: String,
}

impl ScanClient {
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

    async fn text(&self, url: &str) -> Result<String> {
        match self.outside.get(url, None).await.map_err(down)? {
            Fetched::Body { text, .. } => Ok(text),
            other => Err(down(format!("{url}: no answer ({other:?})"))),
        }
    }

    /// The SCAN stations of the contiguous United States.
    // @spec WX-SRC-SCAN-001, WX-SRC-ERR-001
    pub async fn catalog(&self) -> Result<Vec<Station>> {
        // Asking for every element of every station is refused as too large, so the list
        // is narrowed to the hourly elements the record has inputs for.
        let url = format!(
            "{}/stations?stationTriplets=*:*:SCAN&returnStationElements=true\
             &elements=TOBS,RHUM,PRCP,STO:*,SMS:*&durations=HOURLY",
            self.base
        );
        let stations = parse::catalog(&self.text(&url).await?).map_err(down)?;
        if stations.is_empty() {
            return Err(down(format!("{url}: the list held no SCAN stations")));
        }
        Ok(stations)
    }

    /// One station's hours from `from` through `through` (at most a year), in UTC.
    // @spec WX-SRC-SCAN-002, WX-SRC-SCAN-003, WX-SRC-ERR-001
    pub async fn hourly(
        &self,
        station: &Station,
        from: NaiveDate,
        through: NaiveDate,
    ) -> Result<Vec<HourlyRow>> {
        if (through - from).num_days() > MAX_SPAN_DAYS {
            return Err(TurfOpsError::Internal(format!(
                "SCAN is asked for at most a year at a time, not {from} to {through}"
            )));
        }
        let (Some(triplet), Some(offset)) = (&station.locator, station.utc_offset_hours) else {
            return Err(TurfOpsError::Internal(format!(
                "SCAN station {} has no triplet or time offset in the catalog",
                station.station_id
            )));
        };
        let url = format!(
            "{}/data?stationTriplets={triplet}&elements={ELEMENTS}&duration=HOURLY\
             &beginDate={from}%2000:00&endDate={through}%2023:00",
            self.base
        );
        parse::hourly(&self.text(&url).await?, offset).map_err(down)
    }
}

fn down(detail: String) -> TurfOpsError {
    TurfOpsError::unavailable(Source::SoilStation, detail)
}
