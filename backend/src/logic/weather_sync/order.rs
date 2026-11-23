//! The order due fetches run in, and when a failed one may be tried again.

#[cfg(test)]
#[path = "order_tests.rs"]
mod tests;

use crate::datasources::cadence::Product;
use crate::logic::weather_record::due::{Fetch, Purpose};
use crate::models::weather_record::SourceRole;
use chrono::{DateTime, Datelike, Duration, Utc};

/// Where a current fetch stands in a pass: hourly data before daily, the lawn's own
/// sources before the ones that calibrate the estimate.
// @spec SYNC-SCHED-003
fn current_place(fetch: &Fetch) -> (u8, u8) {
    let grain = match fetch.product {
        Product::StationHours | Product::AnalysisHours => 0,
        Product::PrismDays | Product::NrccDays => 1,
        _ => 2,
    };
    (grain, role_place(fetch))
}

fn role_place(fetch: &Fetch) -> u8 {
    match fetch.source.role {
        SourceRole::Basis => 0,
        SourceRole::Calibration => 1,
    }
}

/// The current fetches in the order they run, then the backfill: newest year first
/// and, within a year, the lawn's own sources before calibration sources.
// @spec SYNC-SCHED-003, SYNC-SCHED-004
pub fn ordered(fetches: Vec<Fetch>) -> (Vec<Fetch>, Vec<Fetch>) {
    let (mut current, mut backfill): (Vec<Fetch>, Vec<Fetch>) = fetches
        .into_iter()
        .partition(|fetch| fetch.purpose == Purpose::Current);
    current.sort_by_key(current_place);
    backfill.sort_by_key(|fetch| (std::cmp::Reverse(fetch.span.1.year()), role_place(fetch)));
    (current, backfill)
}

/// How long after its latest failure something may be tried again: 5 minutes after
/// the first failure in a row, 15 after the second, then an hour.
// @spec SYNC-SCHED-006
pub fn retry_after(failures_in_a_row: u32) -> Duration {
    match failures_in_a_row {
        0 => Duration::zero(),
        1 => Duration::minutes(5),
        2 => Duration::minutes(15),
        _ => Duration::hours(1),
    }
}

/// How something that is fetched has been failing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Failing {
    pub in_a_row: u32,
    pub last: Option<DateTime<Utc>>,
}

impl Failing {
    pub fn failed(&mut self, at: DateTime<Utc>) {
        self.in_a_row += 1;
        self.last = Some(at);
    }

    pub fn succeeded(&mut self) {
        *self = Self::default();
    }

    /// Whether it may be tried at `now`.
    // @spec SYNC-SCHED-006
    pub fn may_try(&self, now: DateTime<Utc>) -> bool {
        self.last
            .is_none_or(|last| now - last >= retry_after(self.in_a_row))
    }
}
