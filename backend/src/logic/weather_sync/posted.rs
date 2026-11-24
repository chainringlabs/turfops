//! Posted sensor readings (WX-SRC-POST) before they touch the database: reading the
//! request body, each reading's own problems, the order they are taken in, and what is
//! logged. No I/O.

#[cfg(test)]
#[path = "posted_tests.rs"]
mod tests;

use super::sensor_reading::record_instant;
use chrono::{DateTime, Utc};
use std::collections::HashSet;

/// One reading as posted, its fields checked.
#[derive(Debug, Clone, PartialEq)]
pub struct PostedReading {
    /// Where it was in the request, from 0.
    pub index: usize,
    /// The sensor name as sent, trimmed.
    pub sensor: String,
    pub value: f64,
    pub unit: String,
    pub observed_at: DateTime<Utc>,
}

/// A reading whose own fields make it unusable.
#[derive(Debug, Clone, PartialEq)]
pub struct BadReading {
    pub index: usize,
    /// The sensor name as sent, when there was one.
    pub sensor: Option<String>,
    pub problem: String,
}

/// The whole body is unusable: not `{"readings": [...]}` with at least one entry.
#[derive(Debug, Clone, PartialEq)]
pub struct BadBody(pub String);

/// The readings of a request body, each usable or not. A reading with no time observed
/// takes `received`.
// @spec WX-SRC-POST-001, WX-SRC-POST-004, WX-SRC-POST-005, WX-SRC-POST-015
pub fn readings(
    body: &serde_json::Value,
    received: DateTime<Utc>,
) -> Result<Vec<Result<PostedReading, BadReading>>, BadBody> {
    let entries = body
        .get("readings")
        .and_then(serde_json::Value::as_array)
        .filter(|entries| !entries.is_empty())
        .ok_or_else(|| {
            BadBody("Send readings as {\"readings\": [...]} with at least one reading.".into())
        })?;
    Ok(entries
        .iter()
        .enumerate()
        .map(|(index, entry)| reading(index, entry, received))
        .collect())
}

/// One entry of the `readings` list.
fn reading(
    index: usize,
    entry: &serde_json::Value,
    received: DateTime<Utc>,
) -> Result<PostedReading, BadReading> {
    let sensor = entry
        .get("sensor")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    let bad = |problem: String| BadReading {
        index,
        sensor: sensor.clone(),
        problem,
    };
    let Some(name) = sensor.clone() else {
        return Err(bad(
            "A reading needs the name of its sensor in \"sensor\".".into()
        ));
    };
    let Some(value) = entry.get("value").and_then(serde_json::Value::as_f64) else {
        return Err(bad(format!(
            "The reading for {name} needs \"value\" as a number."
        )));
    };
    let Some(unit) = entry.get("unit").and_then(serde_json::Value::as_str) else {
        return Err(bad(format!("The reading for {name} needs its \"unit\".")));
    };
    let observed_at = match entry.get("observed_at") {
        None | Some(serde_json::Value::Null) => received,
        Some(time) => time
            .as_str()
            .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
            .map(|time| time.with_timezone(&Utc))
            .ok_or_else(|| {
                bad(format!(
                    "The reading for {name} gives \"observed_at\" without a UTC offset; send \
                     it as RFC 3339, for example 2026-11-24T15:00:00Z."
                ))
            })?,
    };
    Ok(PostedReading {
        index,
        sensor: name,
        value,
        unit: unit.trim().to_string(),
        observed_at: record_instant(observed_at),
    })
}

/// The order readings are taken in: each sensor's oldest first, ties in request order.
// @spec WX-SRC-POST-008
pub fn in_time_order(readings: &mut [PostedReading]) {
    // A stable sort keeps the request's order among readings of the same time.
    readings.sort_by_key(|reading| reading.observed_at);
}

/// Two names for the same sensor: letter case and surrounding space aside.
// @spec WX-SRC-POST-007, SETUP-MAP-012
pub fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// What posted readings have been logged since the server started, so a sensor's first
/// stored reading is logged at INFO and its rejections at WARN once until it next stores
/// one.
#[derive(Debug, Default)]
pub struct PostedLog {
    announced: HashSet<String>,
    warned: HashSet<String>,
}

impl PostedLog {
    /// A reading for `sensor` was stored. Returns whether to log it at INFO.
    // @spec WX-SRC-POST-012
    pub fn stored(&mut self, sensor: &str) -> bool {
        let key = sensor.trim().to_lowercase();
        self.warned.remove(&key);
        self.announced.insert(key)
    }

    /// A reading for `sensor` was rejected. Returns whether to log it at WARN.
    // @spec WX-SRC-POST-012
    pub fn rejected(&mut self, sensor: &str) -> bool {
        self.warned.insert(sensor.trim().to_lowercase())
    }
}
