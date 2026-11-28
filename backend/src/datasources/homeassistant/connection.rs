//! A Home Assistant the owner connected in the app: testing the connection and listing
//! the sensor entities it has. The token is sent only as a header and never appears in
//! an error.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::time::Duration;

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Why a Home Assistant could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unusable {
    /// The address is not an `http` or `https` URL.
    BadAddress,
    /// Nothing answered there as Home Assistant.
    NoAnswer,
    /// It answered and refused the token.
    Refused,
}

impl Unusable {
    /// The sentence the owner sees.
    // @spec SETUP-FAIL-002, SETUP-HA-003
    pub fn sentence(&self) -> &'static str {
        match self {
            Unusable::BadAddress => {
                "A Home Assistant address starts with http:// or https://, for example \
                 http://homeassistant.local:8123."
            }
            Unusable::NoAnswer => "Home Assistant did not answer at that address.",
            Unusable::Refused => "Home Assistant refused the token.",
        }
    }
}

/// Nothing usable answered at `path`. Why goes to the log; the address, which may carry a
/// user name and password, and the token never do.
// @spec WX-SRC-ERR-001, WX-SRC-ERR-002
fn no_answer(path: &str, why: impl std::fmt::Display) -> Unusable {
    tracing::warn!(path, %why, "Home Assistant did not answer as expected");
    Unusable::NoAnswer
}

/// A request error without the URL it was sent to.
fn plain(error: reqwest::Error) -> reqwest::Error {
    error.without_url()
}

fn refused(path: &str, status: reqwest::StatusCode) -> Unusable {
    tracing::warn!(path, %status, "Home Assistant refused the token");
    Unusable::Refused
}

/// What Home Assistant says about one entity.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup {
    Reading(Entity),
    /// It exists, and its state is not a number just now (`unavailable`, `unknown`).
    NotANumber,
    /// Home Assistant has no such sensor.
    Missing,
}

/// What answered a probe.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct Answered {
    /// The name the installation gives itself, such as "Home".
    #[serde(default)]
    pub location_name: String,
    #[serde(default)]
    pub version: String,
}

/// A numeric sensor entity as Home Assistant reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub entity_id: String,
    pub friendly_name: String,
    pub value: f64,
    pub unit: Option<String>,
    pub device_class: Option<String>,
    pub state_class: Option<String>,
    /// When the device last reported, even if the value did not change.
    pub reported_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct State {
    entity_id: String,
    state: String,
    #[serde(default)]
    attributes: Attributes,
    last_reported: Option<DateTime<Utc>>,
    last_updated: Option<DateTime<Utc>>,
}

#[derive(Deserialize, Default)]
struct Attributes {
    friendly_name: Option<String>,
    unit_of_measurement: Option<String>,
    device_class: Option<String>,
    state_class: Option<String>,
}

impl State {
    /// The entity, when it is a sensor whose state is a number. `unavailable` and
    /// `unknown` are no reading.
    // @spec WX-SRC-HA-001, WX-SRC-HA-002, WX-SRC-HA-004
    fn entity(self) -> Option<Entity> {
        if !self.entity_id.starts_with("sensor.") {
            return None;
        }
        let value = self
            .state
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())?;
        Some(Entity {
            friendly_name: self
                .attributes
                .friendly_name
                .unwrap_or_else(|| self.entity_id.clone()),
            entity_id: self.entity_id,
            value,
            unit: self.attributes.unit_of_measurement,
            device_class: self.attributes.device_class,
            state_class: self.attributes.state_class,
            // Older versions have no `last_reported`; `last_updated` is the nearest thing.
            reported_at: self.last_reported.or(self.last_updated)?,
        })
    }
}

pub struct HaConnection {
    http: reqwest::Client,
    address: String,
    token: String,
}

impl HaConnection {
    /// A connection to the Home Assistant at `address`, an `http` or `https` URL.
    // @spec SETUP-HA-003
    pub fn new(address: &str, token: &str) -> Result<Self, Unusable> {
        let address = address.trim().trim_end_matches('/');
        let parsed = reqwest::Url::parse(address).map_err(|_| Unusable::BadAddress)?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err(Unusable::BadAddress);
        }
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| no_answer("(no client)", plain(e)))?;
        Ok(Self {
            http,
            address: address.to_string(),
            token: token.trim().to_string(),
        })
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response, Unusable> {
        let response = self
            .http
            .get(format!("{}{path}", self.address))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| no_answer(path, plain(e)))?;
        match response.status() {
            status if status.is_success() => Ok(response),
            status @ (reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN) => {
                Err(refused(path, status))
            }
            status => Err(no_answer(path, format!("answered {status}"))),
        }
    }

    /// One authenticated request: did it answer, did it accept the token, and what is it.
    // @spec WX-SRC-HA-005, SETUP-HA-001
    pub async fn probe(&self) -> Result<Answered, Unusable> {
        const PATH: &str = "/api/config";
        self.get(PATH)
            .await?
            .json()
            .await
            .map_err(|e| no_answer(PATH, plain(e)))
    }

    /// One entity by id: its reading, or why there is none.
    // @spec WX-SRC-HA-001, WX-SRC-HA-002
    pub async fn entity(&self, entity_id: &str) -> Result<Lookup, Unusable> {
        // An id is a domain and a name of lowercase letters, digits and underscores;
        // anything else is not sent, so it cannot reach another path.
        let well_formed = entity_id.split_once('.').is_some_and(|(domain, name)| {
            let plain = |part: &str| {
                !part.is_empty()
                    && part
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            };
            plain(domain) && plain(name)
        });
        if !well_formed {
            return Ok(Lookup::Missing);
        }
        let path = format!("/api/states/{entity_id}");
        let response = self
            .http
            .get(format!("{}{path}", self.address))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|e| no_answer(&path, plain(e)))?;
        match response.status() {
            reqwest::StatusCode::NOT_FOUND => Ok(Lookup::Missing),
            status @ (reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN) => {
                Err(refused(&path, status))
            }
            status if status.is_success() => {
                let state: State = response
                    .json()
                    .await
                    .map_err(|e| no_answer(&path, plain(e)))?;
                Ok(state.entity().map_or(Lookup::NotANumber, Lookup::Reading))
            }
            status => Err(no_answer(&path, format!("answered {status}"))),
        }
    }

    /// Every sensor entity whose state is a number.
    // @spec WX-SRC-HA-004
    pub async fn entities(&self) -> Result<Vec<Entity>, Unusable> {
        const PATH: &str = "/api/states";
        let states: Vec<State> = self
            .get(PATH)
            .await?
            .json()
            .await
            .map_err(|e| no_answer(PATH, plain(e)))?;
        Ok(states.into_iter().filter_map(State::entity).collect())
    }
}
