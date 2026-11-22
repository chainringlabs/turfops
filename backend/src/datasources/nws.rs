//! The National Weather Service as a forecast provider (WX-SRC-FC): keyless hourly
//! points for about a week, with rain amounts for about the first three days.

mod parse;
#[cfg(test)]
mod tests;

pub use parse::parse;

use super::outside::{Fetched, Outside};
use crate::clock::Clock;
use crate::error::{Result, Source, TurfOpsError};
use crate::models::forecast::{ForecastProvider, WeatherForecast};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

const BASE_URL: &str = "https://api.weather.gov";

#[derive(Deserialize)]
struct Point {
    properties: Cell,
}

/// The forecast grid cell that holds a point.
#[derive(Deserialize)]
struct Cell {
    #[serde(rename = "gridId")]
    office: String,
    #[serde(rename = "gridX")]
    x: u32,
    #[serde(rename = "gridY")]
    y: u32,
}

pub struct NwsClient {
    outside: Arc<Outside>,
    base_url: String,
    /// The last point asked about, to four decimal places, and its cell's forecast URL.
    cell: Mutex<Option<(String, String)>>,
}

impl NwsClient {
    pub fn new(outside: Arc<Outside>) -> Self {
        Self {
            outside,
            base_url: BASE_URL.into(),
            cell: Mutex::new(None),
        }
    }

    #[cfg(test)]
    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// The forecast for a point, from the hour in progress at `now`.
    // @spec WX-SRC-FC-006, WX-SRC-FC-008, WX-SRC-REQ-006, WX-SRC-ERR-001
    pub async fn forecast(
        &self,
        latitude: f64,
        longitude: f64,
        clock: &Clock,
        now: DateTime<Utc>,
    ) -> Result<WeatherForecast> {
        let url = self.cell_url(latitude, longitude).await.map_err(down)?;
        let text = self.body(&url).await.map_err(down)?;
        let hourly = parse(&text, now).map_err(down)?;
        Ok(WeatherForecast::new(
            ForecastProvider::NationalWeatherService,
            now,
            hourly,
            clock,
        ))
    }

    /// The forecast URL of the grid cell holding the point; asked once per point.
    // @spec WX-SRC-FC-007
    async fn cell_url(&self, latitude: f64, longitude: f64) -> std::result::Result<String, String> {
        // The service redirects a point given to more than four decimal places.
        let point = format!("{latitude:.4},{longitude:.4}");
        let held = self.cell.lock().expect("the cell is not poisoned").clone();
        if let Some((_, url)) = held.filter(|(held_point, _)| *held_point == point) {
            return Ok(url);
        }
        let text = self
            .body(&format!("{}/points/{point}", self.base_url))
            .await?;
        let cell = serde_json::from_str::<Point>(&text)
            .map_err(|e| format!("NWS point did not parse: {e}"))?
            .properties;
        // The URL is built here, not taken from the response, so a response cannot
        // send the next request somewhere else.
        if cell.office.is_empty() || !cell.office.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(format!("NWS office {:?} is not a name", cell.office));
        }
        let url = format!(
            "{}/gridpoints/{}/{},{}",
            self.base_url, cell.office, cell.x, cell.y
        );
        *self.cell.lock().expect("the cell is not poisoned") = Some((point, url.clone()));
        Ok(url)
    }

    async fn body(&self, url: &str) -> std::result::Result<String, String> {
        match self.outside.get(url, None).await? {
            Fetched::Body { text, .. } => Ok(text),
            Fetched::NotModified | Fetched::Missing => Err(format!("{url} returned no body")),
        }
    }
}

fn down(detail: String) -> TurfOpsError {
    TurfOpsError::unavailable(Source::Forecast, detail)
}
