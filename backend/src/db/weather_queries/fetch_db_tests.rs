//! Fetch state against PostgreSQL: what is wanted, what is held, how fetches went.

use super::{fetch_state, rows, sources};
use crate::models::weather_record::*;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use sqlx::PgPool;

fn d(month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, month, day).unwrap()
}

fn at(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 3, hour, 0, 0).unwrap()
}

async fn source(pool: &PgPool, key: &str, kind: SourceKind) -> Source {
    let new = NewSource {
        key: key.into(),
        kind,
        name: key.into(),
        station_ref: None,
        latitude: None,
        longitude: None,
        distance_miles: None,
        role: SourceRole::Basis,
    };
    sources::ensure(pool, &new).await.unwrap()
}

async fn state(pool: &PgPool) -> FetchState {
    fetch_state::all(pool).await.unwrap().remove(0)
}

// @spec WX-REC-FILL-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_wanted_product_starts_with_nothing_held_and_no_history(pool: PgPool) {
    let station = source(&pool, "station:Uscrn:3761", SourceKind::Station).await;
    fetch_state::want(&pool, station.id, "hourly", d(1, 1))
        .await
        .unwrap();
    let state = state(&pool).await;
    assert_eq!(
        (state.source_id, state.product.as_str()),
        (station.id, "hourly")
    );
    assert_eq!((state.wanted_from, state.held), (d(1, 1), None));
    assert_eq!(
        (state.last_attempt, state.last_success, state.last_failure),
        (None, None, None)
    );
}

// @spec WX-REC-FILL-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_successful_fetch_writes_its_rows_and_widens_what_is_held(pool: PgPool) {
    let station = source(&pool, "station:Uscrn:3761", SourceKind::Station).await;
    fetch_state::want(&pool, station.id, "hourly", d(1, 1))
        .await
        .unwrap();
    let row = HourlyRow {
        observed_at: at(5),
        air_temp_f: Some(61.5),
        ..Default::default()
    };
    fetch_state::hourly_fetched(
        &pool,
        station.id,
        "hourly",
        &[row],
        (d(6, 1), d(6, 3)),
        at(6),
    )
    .await
    .unwrap();
    let held = rows::hourly(&pool, station.id, at(0), at(23))
        .await
        .unwrap();
    assert_eq!(held.len(), 1);
    let after = state(&pool).await;
    assert_eq!(after.held, Some((d(6, 1), d(6, 3))));
    assert_eq!(
        (after.last_attempt, after.last_success),
        (Some(at(6)), Some(at(6)))
    );

    // Backfill reaches earlier; a later update reaches further on.
    fetch_state::hourly_fetched(&pool, station.id, "hourly", &[], (d(5, 1), d(5, 31)), at(7))
        .await
        .unwrap();
    fetch_state::hourly_fetched(&pool, station.id, "hourly", &[], (d(6, 2), d(6, 4)), at(8))
        .await
        .unwrap();
    assert_eq!(state(&pool).await.held, Some((d(5, 1), d(6, 4))));
}

// @spec WX-REC-FILL-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_failed_fetch_is_recorded_and_leaves_what_is_held_unchanged(pool: PgPool) {
    let station = source(&pool, "station:Uscrn:3761", SourceKind::Station).await;
    fetch_state::want(&pool, station.id, "hourly", d(1, 1))
        .await
        .unwrap();
    fetch_state::hourly_fetched(&pool, station.id, "hourly", &[], (d(6, 1), d(6, 3)), at(6))
        .await
        .unwrap();
    fetch_state::failed(&pool, station.id, "hourly", "timed out", at(9))
        .await
        .unwrap();
    let after = state(&pool).await;
    assert_eq!(after.held, Some((d(6, 1), d(6, 3))));
    assert_eq!(after.last_success, Some(at(6)));
    assert_eq!(
        (after.last_attempt, after.last_failure),
        (Some(at(9)), Some(at(9)))
    );
    assert_eq!(after.last_error.as_deref(), Some("timed out"));
}

// @spec WX-REC-FILL-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn rows_that_cannot_be_written_leave_the_held_span_where_it_was(pool: PgPool) {
    // An hourly source handed daily rows: the write is refused, and with it the span.
    let station = source(&pool, "station:Uscrn:3761", SourceKind::Station).await;
    fetch_state::want(&pool, station.id, "daily", d(1, 1))
        .await
        .unwrap();
    let rows_in = [DailyRow {
        day: d(6, 1),
        ..Default::default()
    }];
    let refused = fetch_state::daily_fetched(
        &pool,
        &station,
        "daily",
        &rows_in,
        (d(6, 1), d(6, 1)),
        at(6),
    )
    .await;
    assert!(refused.is_err());
    let after = state(&pool).await;
    assert_eq!((after.held, after.last_success), (None, None));
}

// @spec WX-REC-FILL-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn wanting_a_product_again_moves_its_start_and_keeps_what_it_holds(pool: PgPool) {
    let grid = source(&pool, "grid:Prism:1", SourceKind::Grid).await;
    fetch_state::want(&pool, grid.id, "daily", d(3, 1))
        .await
        .unwrap();
    let rows_in = [DailyRow {
        day: d(6, 1),
        air_max_f: Some(80.0),
        ..Default::default()
    }];
    fetch_state::daily_fetched(&pool, &grid, "daily", &rows_in, (d(6, 1), d(6, 1)), at(6))
        .await
        .unwrap();
    fetch_state::want(&pool, grid.id, "daily", d(1, 1))
        .await
        .unwrap();
    let after = state(&pool).await;
    assert_eq!(
        (after.wanted_from, after.held),
        (d(1, 1), Some((d(6, 1), d(6, 1))))
    );
}

// @spec WX-REC-STORE-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn removing_a_source_removes_everything_stored_under_it(pool: PgPool) {
    let grid = source(&pool, "grid:Prism:1", SourceKind::Grid).await;
    fetch_state::want(&pool, grid.id, "daily", d(1, 1))
        .await
        .unwrap();
    let rows_in = [DailyRow {
        day: d(6, 1),
        air_max_f: Some(80.0),
        ..Default::default()
    }];
    fetch_state::daily_fetched(&pool, &grid, "daily", &rows_in, (d(6, 1), d(6, 1)), at(6))
        .await
        .unwrap();
    sources::delete(&pool, grid.id).await.unwrap();
    assert!(fetch_state::all(&pool).await.unwrap().is_empty());
    assert!(rows::daily(&pool, grid.id, d(1, 1), d(12, 31))
        .await
        .unwrap()
        .is_empty());
}
