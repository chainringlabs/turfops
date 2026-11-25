//! When the memo is reused, and what a failed read serves.

use super::*;
use chrono::TimeZone;

fn at(minute: u32, second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 24, 18, minute, second)
        .unwrap()
}

fn summary(ambient: f64) -> EnvironmentalSummary {
    EnvironmentalSummary {
        ambient_temp_7day_avg_f: Some(ambient),
        last_updated: Some(at(0, 0)),
        ..Default::default()
    }
}

// @spec SYNC-MERGE-008
#[test]
fn a_summary_is_reused_for_a_minute_while_no_pass_changed_current_data() {
    let mut kept = Kept::default();
    assert!(kept.reuse(at(0, 0), 7).is_none(), "nothing assembled yet");
    kept.assembled(summary(44.0), at(0, 0), 7);
    let reused = kept.reuse(at(0, 59), 7).expect("within the minute");
    assert_eq!(reused.ambient_temp_7day_avg_f, Some(44.0));
    assert!(kept.reuse(at(1, 0), 7).is_none(), "a minute old");
    assert!(
        kept.reuse(at(0, 30), 8).is_none(),
        "a pass changed current data"
    );
}

// @spec SYNC-MERGE-008, SYNC-SCHED-009
#[test]
fn a_discarded_memo_is_not_reused() {
    let mut kept = Kept::default();
    kept.assembled(summary(44.0), at(0, 0), 7);
    kept.discard();
    assert!(kept.reuse(at(0, 10), 7).is_none());
}

// @spec SYNC-FAIL-010
#[test]
fn a_failed_read_serves_the_last_good_summary_for_five_minutes() {
    let mut kept = Kept::default();
    kept.assembled(summary(44.0), at(0, 0), 7);
    kept.discard();
    let served = kept.failed(at(4, 59));
    assert_eq!(served.ambient_temp_7day_avg_f, Some(44.0));

    let emptied = kept.failed(at(5, 0));
    assert_eq!(emptied.ambient_temp_7day_avg_f, None);
    assert!(emptied.current.is_none() && emptied.current_bases.is_empty());
    assert_eq!(emptied.last_updated, Some(at(5, 0)));

    let never = Kept::default().failed(at(1, 0));
    assert_eq!(never.ambient_temp_7day_avg_f, None);
    assert_eq!(never.last_updated, Some(at(1, 0)));
}
