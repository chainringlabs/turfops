//! The place search used in setup: a typed name or postal code to candidate places.

#[cfg(test)]
#[path = "places_tests.rs"]
mod tests;

use super::OpenMeteoClient;
use crate::error::{Result, Source, TurfOpsError};
use serde::{Deserialize, Serialize};

const MAX_CANDIDATES: &str = "10";
const UNITED_STATES: &str = "US";

/// A place the owner may choose as the lawn's location.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Place {
    pub name: String,
    pub state: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Deserialize)]
struct Body {
    #[serde(default)]
    results: Vec<Found>,
}

#[derive(Deserialize)]
struct Found {
    name: String,
    latitude: f64,
    longitude: f64,
    country_code: Option<String>,
    admin1: Option<String>,
}

impl OpenMeteoClient {
    /// Candidate places in the United States for what the owner typed. The typed text is
    /// the only thing sent.
    // @spec WX-SRC-OM-004, WX-SRC-ERR-001
    pub async fn places(&self, typed: &str) -> Result<Vec<Place>> {
        let typed = typed.trim();
        if typed.is_empty() {
            return Ok(Vec::new());
        }
        let url = reqwest::Url::parse_with_params(
            &self.search_url,
            [
                ("name", typed),
                ("count", MAX_CANDIDATES),
                ("language", "en"),
                ("format", "json"),
                ("countryCode", UNITED_STATES),
            ],
        )
        .map_err(|e| search_down(format!("place search URL: {e}")))?;
        let text = self.body(url.as_str()).await.map_err(search_down)?;
        parse(&text).map_err(search_down)
    }
}

fn search_down(detail: String) -> TurfOpsError {
    TurfOpsError::unavailable(Source::PlaceSearch, detail)
}

// @spec WX-SRC-OM-004
pub fn parse(json: &str) -> std::result::Result<Vec<Place>, String> {
    let body: Body =
        serde_json::from_str(json).map_err(|e| format!("place search did not parse: {e}"))?;
    Ok(body
        .results
        .into_iter()
        .filter(|found| found.country_code.as_deref() == Some(UNITED_STATES))
        .map(|found| Place {
            name: found.name,
            state: found.admin1,
            latitude: found.latitude,
            longitude: found.longitude,
        })
        .collect())
}
