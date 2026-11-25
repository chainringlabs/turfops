//! Keeping the weather record current (SYNC): running the fetches the record says are
//! due and writing what they return.

pub mod fetcher;
pub mod forecast;

use crate::logic::weather_record::record::AgeLimits;
use chrono::Duration;

/// How old a source's newest value may be and still be current.
// @spec SYNC-AGE-001
pub const AGE_LIMITS: AgeLimits = AgeLimits {
    sensor_air: Duration::hours(1),
    sensor_soil: Duration::hours(6),
    analysis: Duration::hours(3),
    station: Duration::hours(72),
    daily: Duration::hours(72),
};
pub mod order;
pub mod planning;
pub mod posted;
pub mod schedule;
pub mod sensor_read;
pub mod sensor_reading;
pub mod sensor_store;

#[cfg(all(test, feature = "db-tests"))]
mod fetcher_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod forecast_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod schedule_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod sensor_read_db_tests;

/// Clients whose every outside source is unreachable (tests).
#[cfg(all(test, feature = "db-tests"))]
pub(crate) fn unreachable_clients() -> fetcher::Clients {
    schedule_db_tests::unreachable()
}
