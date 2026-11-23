//! The order of a pass, and the wait after a failure.

use super::*;
use crate::models::weather_record::{Source, SourceKind};
use chrono::{NaiveDate, TimeZone};

fn fetch(key: &str, product: Product, role: SourceRole, purpose: Purpose, year: i32) -> Fetch {
    let day = |m, d| NaiveDate::from_ymd_opt(year, m, d).unwrap();
    Fetch {
        source: Source {
            id: 0,
            key: key.into(),
            kind: SourceKind::Station,
            name: key.into(),
            station_ref: None,
            latitude: None,
            longitude: None,
            distance_miles: None,
            role,
        },
        product,
        span: (day(1, 1), day(12, 31)),
        purpose,
    }
}

fn keys(fetches: &[Fetch]) -> Vec<&str> {
    fetches.iter().map(|f| f.source.key.as_str()).collect()
}

// @spec SYNC-SCHED-003
#[test]
fn current_hourly_data_runs_before_daily_and_the_lawns_own_before_calibration() {
    use Product::*;
    use SourceRole::*;
    let now = Purpose::Current;
    let (current, backfill) = ordered(vec![
        fetch("cal-station", CalibrationDays, Calibration, now, 2026),
        fetch("cal-grid", PrismDays, Calibration, now, 2026),
        fetch("nrcc", NrccDays, Basis, now, 2026),
        fetch("prism", PrismDays, Basis, now, 2026),
        fetch("analysis", AnalysisHours, Basis, now, 2026),
        fetch("station", StationHours, Basis, now, 2026),
    ]);
    assert_eq!(
        keys(&current),
        vec![
            "analysis",
            "station",
            "nrcc",
            "prism",
            "cal-grid",
            "cal-station"
        ]
    );
    assert!(backfill.is_empty());
}

// @spec SYNC-SCHED-004
#[test]
fn backfill_is_newest_year_first_and_the_lawns_own_sources_lead_each_year() {
    use Product::*;
    use SourceRole::*;
    let old = Purpose::Backfill;
    let (current, backfill) = ordered(vec![
        fetch("cal 2024", CalibrationDays, Calibration, old, 2024),
        fetch("station 2023", StationHours, Basis, old, 2023),
        fetch("cal 2025", CalibrationDays, Calibration, old, 2025),
        fetch("station 2025", StationHours, Basis, old, 2025),
        fetch("prism 2024", PrismDays, Basis, old, 2024),
        fetch("now", StationHours, Basis, Purpose::Current, 2026),
    ]);
    assert_eq!(keys(&current), vec!["now"]);
    assert_eq!(
        keys(&backfill),
        vec![
            "station 2025",
            "cal 2025",
            "prism 2024",
            "cal 2024",
            "station 2023"
        ]
    );
}

// @spec SYNC-SCHED-006
#[test]
fn a_failure_is_retried_after_5_minutes_then_15_then_hourly() {
    let at = |minutes: i64| {
        Utc.with_ymd_and_hms(2026, 7, 20, 12, 0, 0).unwrap() + Duration::minutes(minutes)
    };
    let mut failing = Failing::default();
    assert!(failing.may_try(at(0)), "nothing has failed");
    failing.failed(at(0));
    assert!(!failing.may_try(at(4)) && failing.may_try(at(5)));
    failing.failed(at(5));
    assert!(!failing.may_try(at(19)) && failing.may_try(at(20)));
    failing.failed(at(20));
    assert!(!failing.may_try(at(79)) && failing.may_try(at(80)));
    failing.failed(at(80));
    assert!(
        !failing.may_try(at(139)) && failing.may_try(at(140)),
        "hourly from then on"
    );
    failing.succeeded();
    assert!(failing.may_try(at(81)));
}
