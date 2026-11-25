//! `GET /api/v1/timing-windows` against a real database and the weather record.

use super::db_harness::{app_state, climate, forecast_body, lawn, ZONE};
use super::*;
use crate::models::weather_basis::BasisKind;
use crate::test_support::stub_http;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use sqlx::PgPool;

const NO_FORECAST: &str = "Forecast unavailable";

async fn status_of(state: AppState) -> StatusCode {
    match get_timing_windows(State(state)).await {
        Ok(_) => StatusCode::OK,
        Err(e) => e.into_response().status(),
    }
}

/// Six years and this year through yesterday.
async fn six_years(pool: &PgPool) {
    climate(pool, 6, ZONE.today() - Duration::days(1)).await;
}

// @spec TIME-ENG-041
#[sqlx::test(migrations = "src/db/migrations")]
async fn no_profile_is_404_and_the_records_outages_are_its_503s(pool: PgPool) {
    assert_eq!(
        status_of(app_state(&pool, None).await).await,
        StatusCode::NOT_FOUND
    );

    six_years(&pool).await;
    // History still loading: the station's download has not reached what is wanted.
    sqlx::query("UPDATE weather_fetch_state SET held_from = NULL, held_through = NULL")
        .execute(&pool)
        .await
        .unwrap();
    let state = app_state(&pool, None).await;
    let loading = get_timing_windows(State(state.clone())).await.unwrap_err();
    assert!(
        loading.owner_message().contains("history"),
        "{}",
        loading.owner_message()
    );
    assert_eq!(
        loading.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let mut profile = lawn(&pool).await;
    profile.location = None;
    queries::update_lawn_profile(&pool, &profile).await.unwrap();
    let unplaced = get_timing_windows(State(state)).await.unwrap_err();
    assert_eq!(
        unplaced.owner_message(),
        TurfOpsError::location_not_set().owner_message()
    );
}

fn keys(value: &serde_json::Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

// @spec TIME-ENG-040, TIME-ENG-044
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_response_carries_windows_context_series_and_notes(pool: PgPool) {
    six_years(&pool).await;
    let Json(response) = get_timing_windows(State(app_state(&pool, None).await))
        .await
        .unwrap();
    let today = ZONE.today();
    assert_eq!(response.today, today);
    assert_eq!(
        response.basis.soil.as_ref().map(|b| b.kind),
        Some(BasisKind::Station)
    );
    assert_eq!(
        response.basis.air.as_ref().map(|b| b.kind),
        Some(BasisKind::Estimate)
    );
    let jan1 = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
    assert!(!response.basis.spans.is_empty());
    assert!(response.basis.spans.iter().all(|s| s.from >= jan1));
    assert_eq!(response.soil.as_of, Some(today - Duration::days(1)));
    assert!(response.soil.avg_5day_f.is_some());
    assert!(response.freeze.first_fall.is_some() && response.freeze.last_spring.is_some());
    assert_eq!(response.windows.len(), 5);
    assert_eq!(response.history_years.len(), 6);
    let days_in_year = NaiveDate::from_ymd_opt(today.year(), 12, 31)
        .unwrap()
        .ordinal();
    assert_eq!(response.series.len(), days_in_year as usize);
    // No forecast configured: the note says so; everything else is healthy.
    assert_eq!(response.data_notes.len(), 1, "{:?}", response.data_notes);
    assert!(response.data_notes[0].starts_with(NO_FORECAST));

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(
        keys(&json),
        vec![
            "basis",
            "context",
            "data_notes",
            "freeze",
            "generated_at",
            "history_years",
            "series",
            "soil",
            "today",
            "windows"
        ]
    );
    let window = &json["windows"][0];
    assert_eq!(
        keys(window),
        vec![
            "closes",
            "conflict",
            "detail",
            "done_on",
            "guidance",
            "headline",
            "id",
            "ideal_from",
            "ideal_until",
            "name",
            "opens",
            "priority",
            "season_year",
            "state"
        ]
    );
    assert_eq!(
        keys(&window["opens"]),
        vec![
            "date",
            "days_held",
            "estimated",
            "label",
            "passed",
            "source",
            "typical",
            "typical_error_days"
        ]
    );
    assert_eq!(
        keys(&json["context"]),
        vec![
            "air_anomaly_30d_f",
            "forecast_alerts",
            "soil_anomaly_7d_f",
            "summary"
        ]
    );
}

/// Row counts of every table in the public schema.
async fn table_counts(pool: &PgPool) -> Vec<(String, i64)> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_type = 'BASE TABLE' ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut counts = Vec::new();
    for table in tables {
        let n: i64 = // Table names come from the catalog, not from input.
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM \"{table}\"")))
            .fetch_one(pool)
            .await
            .unwrap();
        counts.push((table, n));
    }
    counts
}

// @spec TIME-ENG-038, SYNC-MEMO-001, SYNC-MEMO-002, TIME-ENG-046
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_climate_record_is_memoized_in_memory_not_in_postgres(pool: PgPool) {
    six_years(&pool).await;
    let state = app_state(&pool, None).await;
    let before = table_counts(&pool).await;
    let Json(first) = get_timing_windows(State(state.clone())).await.unwrap();
    assert_eq!(first.windows.len(), 5);
    assert!(state.climate_cache.read().await.is_some(), "held in memory");
    assert_eq!(table_counts(&pool).await, before, "nothing stored");

    // A record read under another basis plan is not served: it is read again.
    let held = state.climate_cache.read().await.clone().unwrap().2;
    if let Some((key, _, _)) = state.climate_cache.write().await.as_mut() {
        *key = "another plan".into();
    }
    let Json(_) = get_timing_windows(State(state.clone())).await.unwrap();
    let reread = state.climate_cache.read().await.clone().unwrap();
    assert!(!Arc::ptr_eq(&held, &reread.2), "re-read under the new plan");
    assert_ne!(reread.0, "another plan");

    // Within the hour the memoized record answers even when the record cannot: the lawn
    // has lost its location.
    let mut profile = lawn(&pool).await;
    profile.location = None;
    queries::update_lawn_profile(&pool, &profile).await.unwrap();
    let Json(again) = get_timing_windows(State(state.clone())).await.unwrap();
    assert_eq!(again.history_years.len(), 6);
    assert!(again
        .data_notes
        .iter()
        .all(|n| !n.contains("last good read")));

    // An hour on, the record is read again, fails, and the last good read stands in.
    if let Some((_, read_at, _)) = state.climate_cache.write().await.as_mut() {
        *read_at -= chrono::Duration::minutes(61);
    }
    let Json(stale) = get_timing_windows(State(state)).await.unwrap();
    assert!(
        stale.data_notes[0].contains("last good read"),
        "{:?}",
        stale.data_notes
    );
}

// @spec TIME-ENG-045
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_failed_forecast_fetch_still_answers_without_the_forecast(pool: PgPool) {
    six_years(&pool).await;
    let base = stub_http(500, r#"{"cod":500,"message":"upstream down"}"#).await;
    let state = app_state(&pool, Some(&base)).await;
    let Json(response) = get_timing_windows(State(state)).await.unwrap();
    assert_eq!(response.windows.len(), 5);
    assert!(response.series.iter().all(|d| d.forecast.is_none()));
    assert!(
        response
            .data_notes
            .iter()
            .any(|n| n.starts_with(NO_FORECAST)),
        "{:?}",
        response.data_notes
    );
}

const MILD: [f64; 8] = [50.0, 48.0, 52.0, 60.0, 66.0, 64.0, 58.0, 54.0];

// @spec TIME-ENG-009
#[sqlx::test(migrations = "src/db/migrations")]
async fn only_full_forecast_days_reach_the_windows(pool: PgPool) {
    six_years(&pool).await;
    let alerts = |body: &'static str| {
        let pool = pool.clone();
        async move {
            let base = stub_http(200, body).await;
            let state = app_state(&pool, Some(&base)).await;
            let Json(r) = get_timing_windows(State(state)).await.unwrap();
            assert!(r.data_notes.iter().all(|n| !n.starts_with(NO_FORECAST)));
            r.context.forecast_alerts
        }
    };

    // Two full mild days, then a partial day (two 3-hour points) with a frost and heat.
    let partial = forecast_body(&[
        (1, MILD.to_vec()),
        (2, MILD.to_vec()),
        (3, vec![25.0, 95.0]),
    ]);
    assert!(alerts(partial).await.is_empty());

    // The same frost and heat inside a full day are reported.
    let mut extreme = MILD.to_vec();
    extreme[1] = 25.0;
    extreme[5] = 95.0;
    let full = forecast_body(&[(1, MILD.to_vec()), (2, extreme)]);
    assert_eq!(alerts(full).await.len(), 2);
}
