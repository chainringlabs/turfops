//! What one Home Assistant answer means for a mapped sensor (WX-SRC-HA): a reading to
//! store, a counter's starting point, nothing new, or a problem to show as its status.
//! No I/O: the reader fetches the answer and stores the outcome.

#[cfg(test)]
#[path = "sensor_reading_tests.rs"]
mod tests;

use crate::datasources::homeassistant::connection::Lookup;
use crate::datasources::sensor_units::{accepted_units, convert, plausible};
use crate::db::setup_queries::RainKind;
use crate::models::weather_record::Input;
use chrono::{DateTime, Duration, SubsecRound, Utc};

/// No hour's rain reaches this; a reading of more is not rain (WX-SRC-NORM-002).
pub const RAIN_CEILING_MM: f64 = 300.0;
/// How far ahead of this server's clock a reported time may be.
const CLOCK_SKEW: Duration = Duration::minutes(5);

/// An instant as the record stores it: PostgreSQL keeps microseconds, so a reading's time
/// is cut to them on the way in, or a time with nanoseconds would always look newer than
/// the same time read back.
pub fn record_instant(at: DateTime<Utc>) -> DateTime<Utc> {
    at.trunc_subsecs(6)
}

/// A sensor's previous reading: its value in record units and its reported time. For a
/// rain counter the value is the counter's total, not an amount.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Previous {
    pub value: f64,
    pub at: DateTime<Utc>,
}

/// Why a sensor gave no reading.
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    /// Home Assistant reports a state that is not a number (`unavailable`, `unknown`).
    NotANumber,
    /// Home Assistant has no such entity.
    NotFound,
    /// The entity's unit is not one its sensor's input accepts.
    Unit(Option<String>),
    /// The converted value is not one its input could have.
    Implausible(f64),
    /// The reported time is more than 5 minutes ahead of now.
    Ahead(DateTime<Utc>),
}

impl Problem {
    /// The sentence the owner sees as the status of the sensor mapped to `input` from
    /// `entity_id`.
    // @spec SYNC-SCHED-015
    pub fn sentence(&self, input: Input, entity_id: &str) -> String {
        let label = input.label().to_lowercase();
        match self {
            Problem::NotANumber => {
                format!("Home Assistant reports {entity_id} as unavailable or unknown.")
            }
            Problem::NotFound => format!(
                "{entity_id} was not found in Home Assistant. Remap this sensor or unmap it."
            ),
            Problem::Unit(unit) => format!(
                "This sensor reports in {}. For {label} TurfOps accepts {}.",
                unit.as_deref().unwrap_or("no unit"),
                accepted_units(input)
            ),
            Problem::Implausible(value) => format!(
                "This sensor reported {value}, which is outside what {label} can be. Check \
                 that this is the right sensor."
            ),
            Problem::Ahead(at) => format!(
                "This sensor's reading is dated {}, more than 5 minutes ahead of this \
                 server's clock. Check the clock of whatever sends it.",
                at.format("%b %-d at %H:%M UTC")
            ),
        }
    }
}

/// What to do with one answer.
#[derive(Debug, Clone, PartialEq)]
pub enum Judged {
    /// Store `value` (for rain, the amount) as observed at `at`, and keep `keep` as the
    /// sensor's previous reading. `jumped` marks a counter that rose too far to be rain.
    Store {
        at: DateTime<Utc>,
        value: f64,
        keep: Previous,
        jumped: bool,
    },
    /// A rain counter's first reading: kept as its starting point, nothing stored.
    Starting { keep: Previous },
    /// The reported time is not later than the previous reading's.
    Unchanged,
    /// No reading, for this reason.
    Refused(Problem),
}

/// What a Home Assistant answer for a sensor mapped to `input` means, given its previous
/// reading, at `now`.
// @spec WX-SRC-HA-001, WX-SRC-HA-002, WX-SRC-HA-003, WX-SRC-HA-006, WX-SRC-HA-007, WX-SRC-HA-008
// @spec WX-SRC-HA-009, WX-SRC-HA-010, WX-SRC-HA-013, WX-SRC-HA-014
pub fn judge(
    input: Input,
    rain_kind: Option<RainKind>,
    lookup: Lookup,
    previous: Option<Previous>,
    now: DateTime<Utc>,
) -> Judged {
    let entity = match lookup {
        Lookup::Reading(entity) => entity,
        Lookup::NotANumber => return Judged::Refused(Problem::NotANumber),
        Lookup::Missing => return Judged::Refused(Problem::NotFound),
    };
    let at = record_instant(entity.reported_at);
    if at > now + CLOCK_SKEW {
        return Judged::Refused(Problem::Ahead(at));
    }
    if previous.is_some_and(|previous| at <= previous.at) {
        return Judged::Unchanged;
    }
    let Some(value) = convert(input, entity.value, entity.unit.as_deref()) else {
        return Judged::Refused(Problem::Unit(entity.unit));
    };
    if !plausible_reading(input, rain_kind, value) {
        return Judged::Refused(Problem::Implausible(value));
    }
    let keep = Previous { value, at };
    let counter = input == Input::Rain && rain_kind == Some(RainKind::Counter);
    match (counter, previous) {
        (false, _) => Judged::Store {
            at,
            value,
            keep,
            jumped: false,
        },
        (true, None) => Judged::Starting { keep },
        (true, Some(previous)) => {
            let rise = value - previous.value;
            let jumped = rise > RAIN_CEILING_MM;
            Judged::Store {
                at,
                // A fall is a reset; a jump is a replaced gauge. Neither is rain.
                value: if rise < 0.0 || jumped { 0.0 } else { rise },
                keep,
                jumped,
            }
        }
    }
}

/// Whether one reading is a value its input could have. Rain is judged per reading: an
/// amount up to the hourly ceiling, a counter's total anything not below zero.
fn plausible_reading(input: Input, rain_kind: Option<RainKind>, value: f64) -> bool {
    match (input, rain_kind) {
        (Input::Rain, Some(RainKind::Counter)) => value >= 0.0,
        (Input::Rain, _) => (0.0..=RAIN_CEILING_MM).contains(&value),
        _ => plausible(input, value),
    }
}
