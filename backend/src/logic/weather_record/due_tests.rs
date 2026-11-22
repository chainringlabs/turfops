//! What is due, for a plan of a station, PRISM, the fallback grid and the analysis.

use super::*;
use crate::models::weather_record::SourceRole;
use chrono::TimeZone;

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

const TODAY: (i32, u32, u32) = (2026, 7, 20);

fn today() -> NaiveDate {
    day(TODAY.0, TODAY.1, TODAY.2)
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, 20, 16, 0, 0).unwrap()
}

fn source(id: i64, key: &str, kind: SourceKind) -> Source {
    Source {
        id,
        key: key.into(),
        kind,
        name: key.into(),
        station_ref: None,
        latitude: None,
        longitude: None,
        distance_miles: None,
        role: SourceRole::Basis,
    }
}

fn station() -> Source {
    source(1, "station:Uscrn:a", SourceKind::Station)
}

fn prism() -> Source {
    source(2, "grid:prism:39.8600,-75.7900", SourceKind::Grid)
}

fn nrcc() -> Source {
    source(3, "grid:nrcc:39.8600,-75.7900", SourceKind::Grid)
}

/// A fetch state wanted from `wanted_from`, holding `held`, last fetched `ago` back.
fn state(
    source: &Source,
    wanted_from: NaiveDate,
    held: Option<(NaiveDate, NaiveDate)>,
    ago: Option<Duration>,
) -> Option<FetchState> {
    Some(FetchState {
        source_id: source.id,
        product: product_of(source)?.as_str().into(),
        wanted_from,
        held,
        last_attempt: ago.map(|ago| now() - ago),
        last_success: ago.map(|ago| now() - ago),
        last_failure: None,
        last_error: None,
    })
}

fn due(sources: &[(Source, Option<FetchState>)]) -> Vec<(Purpose, i64, NaiveDate, NaiveDate)> {
    fetches(sources, today(), now())
        .into_iter()
        .map(|f| (f.purpose, f.source.id, f.span.0, f.span.1))
        .collect()
}

// @spec WX-REC-STORE-008
#[test]
fn history_is_wanted_for_15_years_and_the_analysis_from_93_days_before_setup() {
    let located = day(2026, 6, 1);
    assert_eq!(
        wanted_from(Product::StationHours, today(), located),
        day(2011, 1, 1)
    );
    assert_eq!(
        wanted_from(Product::PrismDays, today(), located),
        day(2011, 1, 1)
    );
    assert_eq!(
        wanted_from(Product::AnalysisHours, today(), located),
        day(2026, 2, 28)
    );
}

// @spec WX-REC-FILL-002
#[test]
fn a_new_station_is_due_for_the_year_so_far_then_each_earlier_year_newest_first() {
    let station = station();
    let wanted = day(2023, 1, 1);
    let got = due(&[(station.clone(), state(&station, wanted, None, None))]);
    assert_eq!(
        got,
        vec![
            (Purpose::Current, 1, day(2026, 1, 1), today()),
            (Purpose::Backfill, 1, day(2025, 1, 1), day(2025, 12, 31)),
            (Purpose::Backfill, 1, day(2024, 1, 1), day(2024, 12, 31)),
            (Purpose::Backfill, 1, day(2023, 1, 1), day(2023, 12, 31)),
        ]
    );
}

// @spec WX-REC-FILL-002, WX-REC-FILL-004, WX-SRC-CAD-001
#[test]
fn a_station_is_due_hourly_and_rereads_its_last_three_days() {
    let station = station();
    let held = Some((day(2011, 1, 1), day(2026, 7, 19)));
    let at = |ago| {
        due(&[(
            station.clone(),
            state(&station, day(2011, 1, 1), held, Some(ago)),
        )])
    };
    assert!(at(Duration::minutes(59)).is_empty(), "nothing new yet");
    assert_eq!(
        at(Duration::minutes(60)),
        vec![(Purpose::Current, 1, day(2026, 7, 16), today())]
    );
}

// @spec WX-REC-FILL-002
#[test]
fn after_downtime_the_gap_is_part_of_the_current_fetch() {
    let station = station();
    let held = Some((day(2011, 1, 1), day(2026, 6, 1)));
    let got = due(&[(
        station.clone(),
        state(&station, day(2011, 1, 1), held, Some(Duration::days(49))),
    )]);
    assert_eq!(got, vec![(Purpose::Current, 1, day(2026, 5, 29), today())]);
}

// @spec WX-REC-FILL-002
#[test]
fn an_interrupted_backfill_resumes_below_what_is_held() {
    let station = station();
    let held = Some((day(2024, 1, 1), day(2026, 7, 20)));
    let got = due(&[(
        station.clone(),
        state(&station, day(2022, 6, 1), held, Some(Duration::minutes(5))),
    )]);
    assert_eq!(
        got,
        vec![
            (Purpose::Backfill, 1, day(2023, 1, 1), day(2023, 12, 31)),
            (Purpose::Backfill, 1, day(2022, 6, 1), day(2022, 12, 31)),
        ]
    );
}

// @spec WX-REC-FILL-004, WX-SRC-CAD-001
#[test]
fn the_grid_is_due_daily_and_rereads_its_last_seven_days() {
    let prism = prism();
    let held = Some((day(2011, 1, 1), day(2026, 7, 19)));
    let at = |ago| {
        due(&[(
            prism.clone(),
            state(&prism, day(2011, 1, 1), held, Some(ago)),
        )])
    };
    assert!(at(Duration::hours(23)).is_empty());
    assert_eq!(
        at(Duration::hours(24)),
        vec![(Purpose::Current, 2, day(2026, 7, 12), today())]
    );
}

// @spec WX-SRC-CAD-002
#[test]
fn the_fallback_grid_is_asked_only_for_the_days_prism_has_not_published() {
    let (prism, nrcc) = (prism(), nrcc());
    let fresh = Some(Duration::minutes(5));
    let plan = |prism_held| {
        vec![
            (
                prism.clone(),
                state(&prism, day(2011, 1, 1), prism_held, fresh),
            ),
            (nrcc.clone(), state(&nrcc, today(), None, None)),
        ]
    };
    let behind = due(&plan(Some((day(2011, 1, 1), day(2026, 7, 18)))));
    assert_eq!(
        behind,
        vec![(Purpose::Current, 3, day(2026, 7, 19), today())]
    );
    let caught_up = due(&plan(Some((day(2011, 1, 1), today()))));
    assert!(caught_up.is_empty(), "PRISM has today: {caught_up:?}");
    let never = due(&plan(None));
    let nrcc_only: Vec<_> = never.iter().filter(|f| f.1 == 3).collect();
    assert_eq!(
        nrcc_only,
        vec![&(Purpose::Current, 3, day(2026, 6, 20), today())]
    );
    assert!(never
        .iter()
        .all(|f| !(f.1 == 3 && f.0 == Purpose::Backfill)));
}

// @spec WX-REC-FILL-002
#[test]
fn every_current_fetch_comes_before_any_backfill_and_backfill_is_newest_first_overall() {
    let (station, prism) = (station(), prism());
    let got = due(&[
        (
            station.clone(),
            state(&station, day(2024, 1, 1), None, None),
        ),
        (prism.clone(), state(&prism, day(2024, 1, 1), None, None)),
    ]);
    let purposes: Vec<_> = got.iter().map(|f| f.0).collect();
    assert_eq!(&purposes[..2], &[Purpose::Current, Purpose::Current]);
    let ends: Vec<_> = got[2..].iter().map(|f| f.3).collect();
    let mut sorted = ends.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(ends, sorted);
    assert_eq!(
        got[2],
        (Purpose::Backfill, 2, day(2026, 1, 1), day(2026, 6, 19))
    );
}

// @spec WX-REC-FILL-002
#[test]
fn a_sensor_the_estimate_and_an_unregistered_source_are_not_fetched() {
    let sensor = source(7, "sensor:1", SourceKind::Sensor);
    let estimate = source(8, "estimate:39.8600,-75.7900", SourceKind::Estimate);
    let station = station();
    assert!(due(&[(sensor, None), (estimate, None), (station, None)]).is_empty());
}
