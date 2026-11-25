//! The one-minute memo of the summary and the last good one, kept through a failed read.

use crate::models::EnvironmentalSummary;
use chrono::{DateTime, Duration, Utc};

/// A summary is reused for this long.
pub(crate) const MEMO_FOR: Duration = Duration::minutes(1);
/// After a failed read of the record, the last good summary is served for this long.
pub(crate) const LAST_GOOD_FOR: Duration = Duration::minutes(5);

#[derive(Debug, Clone, Default)]
pub(crate) struct Kept {
    /// The last summary assembled, when, and the schedule's count of passes that
    /// changed current data at that time.
    last: Option<(EnvironmentalSummary, DateTime<Utc>, u64)>,
    /// Whether `last` may be reused as the memo; a discarded one is still the last good.
    fresh: bool,
}

impl Kept {
    /// The memo, while it was assembled under a minute ago and no pass has changed
    /// current data since.
    // @spec SYNC-MERGE-008
    pub fn reuse(&self, now: DateTime<Utc>, changes: u64) -> Option<EnvironmentalSummary> {
        self.last
            .as_ref()
            .filter(|_| self.fresh)
            .filter(|(_, at, seen)| *seen == changes && now - *at < MEMO_FOR)
            .map(|(summary, _, _)| summary.clone())
    }

    pub fn assembled(&mut self, summary: EnvironmentalSummary, at: DateTime<Utc>, changes: u64) {
        self.last = Some((summary, at, changes));
        self.fresh = true;
    }

    /// What to serve when the record cannot be read: the last good summary while it is
    /// under 5 minutes old, else one whose weather fields are empty.
    // @spec SYNC-FAIL-010
    pub fn failed(&self, now: DateTime<Utc>) -> EnvironmentalSummary {
        match &self.last {
            Some((summary, at, _)) if now - *at < LAST_GOOD_FOR => summary.clone(),
            _ => EnvironmentalSummary {
                last_updated: Some(now),
                ..Default::default()
            },
        }
    }

    /// Forget the memo, so the next request assembles afresh.
    pub fn discard(&mut self) {
        self.fresh = false;
    }
}

#[cfg(test)]
#[path = "memo_tests.rs"]
mod tests;
