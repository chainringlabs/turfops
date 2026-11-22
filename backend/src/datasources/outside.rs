//! How TurfOps makes requests to outside weather sources (WX-SRC-REQ): identified by name
//! and version only, bounded in time, one request at a time per host with a pause between
//! them, conditional where a copy is already held, and never retried here.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::Instant;

/// Names the application and its version; carries nothing about the owner.
// @spec WX-SRC-REQ-001
pub const USER_AGENT: &str = concat!("TurfOps/", env!("CARGO_PKG_VERSION"));
// @spec WX-SRC-REQ-002
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// A grid request spanning more than a year.
pub const LONG_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
// @spec WX-SRC-REQ-003
const PAUSE: Duration = Duration::from_secs(1);
/// RCC-ACIS publishes no limits, so it is given more room.
const ACIS_PAUSE: Duration = Duration::from_secs(2);
const ACIS_HOST: &str = "rcc-acis.org";

/// The pause required between two requests to `host`.
// @spec WX-SRC-REQ-003
pub fn pause_for(host: &str) -> Duration {
    if host == ACIS_HOST || host.ends_with(&format!(".{ACIS_HOST}")) {
        ACIS_PAUSE
    } else {
        PAUSE
    }
}

/// What a GET came back with.
#[derive(Debug, Clone, PartialEq)]
pub enum Fetched {
    Body {
        text: String,
        last_modified: Option<DateTime<Utc>>,
    },
    /// The copy already held is current.
    NotModified,
    /// The source has no such file.
    Missing,
}

/// When the last request to a host finished; holding it is holding the host's turn.
type Turn = Arc<tokio::sync::Mutex<Option<Instant>>>;

pub struct Outside {
    http: reqwest::Client,
    turns: Mutex<HashMap<String, Turn>>,
    paced: bool,
}

impl Outside {
    pub fn new() -> Self {
        Self::build(true)
    }

    /// Without pauses between requests, for tests against a local stub.
    #[cfg(test)]
    pub fn unpaced() -> Self {
        Self::build(false)
    }

    fn build(paced: bool) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("the HTTP client builds from constants");
        Self {
            http,
            turns: Mutex::new(HashMap::new()),
            paced,
        }
    }

    /// GET `url`. With `held_since`, asks only for a copy newer than the one held.
    // @spec WX-SRC-REQ-004, WX-SRC-REQ-005
    pub async fn get(
        &self,
        url: &str,
        held_since: Option<DateTime<Utc>>,
    ) -> Result<Fetched, String> {
        let mut request = self.http.get(url);
        if let Some(held) = held_since {
            let stamp = held.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
            request = request.header(reqwest::header::IF_MODIFIED_SINCE, stamp);
        }
        let response = self.send(url, request).await?;
        match response.status() {
            reqwest::StatusCode::NOT_MODIFIED => Ok(Fetched::NotModified),
            reqwest::StatusCode::NOT_FOUND => Ok(Fetched::Missing),
            status if status.is_success() => {
                let last_modified = response
                    .headers()
                    .get(reqwest::header::LAST_MODIFIED)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|text| DateTime::parse_from_rfc2822(text).ok())
                    .map(|stamp| stamp.with_timezone(&Utc));
                let text = response.text().await.map_err(|e| failure(url, e))?;
                Ok(Fetched::Body {
                    text,
                    last_modified,
                })
            }
            status => Err(format!("{url} answered {status}")),
        }
    }

    /// POST a JSON body to `url` and return the response text.
    // @spec WX-SRC-REQ-005
    pub async fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
        timeout: Duration,
    ) -> Result<String, String> {
        let request = self.http.post(url).json(body).timeout(timeout);
        let response = self.send(url, request).await?;
        let status = response.status();
        let text = response.text().await.map_err(|e| failure(url, e))?;
        if status.is_success() {
            Ok(text)
        } else {
            Err(format!("{url} answered {status}: {text}"))
        }
    }

    /// Sends one request once its host's turn comes round. No retry: a failure is returned.
    // @spec WX-SRC-REQ-003, WX-SRC-REQ-005
    async fn send(
        &self,
        url: &str,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, String> {
        let host = reqwest::Url::parse(url)
            .ok()
            .and_then(|parsed| parsed.host_str().map(str::to_string))
            .ok_or_else(|| format!("{url} is not a URL"))?;
        let turn = {
            let mut turns = self.turns.lock().expect("the host table is not poisoned");
            turns.entry(host.clone()).or_default().clone()
        };
        let mut last = turn.lock().await;
        if let (true, Some(finished)) = (self.paced, *last) {
            tokio::time::sleep_until(finished + pause_for(&host)).await;
        }
        let outcome = request.send().await.map_err(|e| failure(url, e));
        *last = Some(Instant::now());
        outcome
    }
}

impl Default for Outside {
    fn default() -> Self {
        Self::new()
    }
}

fn failure(url: &str, error: reqwest::Error) -> String {
    format!("{url}: {}", error.without_url())
}

#[cfg(test)]
mod tests;
