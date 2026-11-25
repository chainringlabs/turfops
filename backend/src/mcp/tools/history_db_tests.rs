//! `weather_history` end to end over the weather record (`--features db-tests`).
//! "Now" is noon Eastern on July 20, 2026.

use crate::logic::weather_record::lab::{d, Lab, PRISM};
use crate::mcp::db_support::{lab_state, lawn};
use crate::mcp::test_support::*;
use serde_json::{json, Value};
use sqlx::PgPool;

fn day(n: u32) -> String {
    d(n).to_string()
}

// @spec MCP-HIST-001, MCP-HIST-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_window_is_thirty_days_by_default_and_clamped_to_one_through_120(pool: PgPool) {
    Lab::new(pool.clone()).await;
    let state = lab_state(pool).await;
    for (days, back) in [
        (json!(null), 30),
        (json!(7), 7),
        (json!(0), 1),
        (json!(-5), 1),
        (json!(500), 120),
    ] {
        let args = if days.is_null() {
            json!({})
        } else {
            json!({"days": days})
        };
        let history = call_tool(&state, "weather_history", args).await.json();
        let start = (d(20) - chrono::Duration::days(back)).to_string();
        assert_eq!(history["start"], start, "days = {days}");
    }
}

/// The analysis's hours on July 16–19 (68 °F, RH 70 %, 0.25 mm an hour), the station's
/// soil on the 17th, and PRISM's daily air on the 14th–19th.
async fn week(pool: PgPool) -> Value {
    let lab = Lab::new(pool.clone()).await;
    for n in 16..=19 {
        lab.analysis_hours(n, 24, 68.0, 70.0, 0.25).await;
    }
    lab.station_hours(17, 24, 62.0).await;
    for n in 14..=19 {
        lab.grid(PRISM, n, 70.0, 52.0).await;
    }
    let state = lab_state(pool).await;
    call_tool(&state, "weather_history", json!({"days": 7}))
        .await
        .json()
}

fn row<'a>(history: &'a Value, date: &str) -> &'a Value {
    history["days"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["date"] == date)
        .unwrap_or_else(|| panic!("no row for {date}: {history}"))
}

// @spec MCP-HIST-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn one_row_per_date_either_source_has_from_start_in_ascending_order(pool: PgPool) {
    let history = week(pool).await;
    let dates: Vec<&str> = history["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["date"].as_str().unwrap())
        .collect();
    let expected = [day(14), day(15), day(16), day(17), day(18), day(19)];
    assert_eq!(dates, expected);
}

// @spec MCP-HIST-003
#[test]
fn the_rows_never_come_from_the_historical_chart_endpoint() {
    let source = include_str!("weather.rs");
    let code = source.split("#[cfg(test)]").next().unwrap();
    assert!(!code.contains("historical::"), "uses /historical");
    assert!(code.contains("fetch_daily_disease_inputs("));
    assert!(code.contains("api::timing::climate_record("));
}

// @spec MCP-HIST-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn each_row_carries_air_humidity_rain_hours_soil_and_gdd(pool: PgPool) {
    let history = week(pool).await;
    assert_eq!(
        row(&history, &day(17)),
        &json!({
            "date": day(17),
            "air_mean_f": 68.0,
            "air_min_f": 68.0,
            "air_max_f": 68.0,
            "rh_mean_pct": 70.0,
            "rain_in": 0.24,
            "hours_observed": 24.0,
            "soil_5cm_f": 64.0,
            "gdd50": 11.0,
        })
    );
}

// @spec MCP-HIST-004, MCP-HIST-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_day_without_hourly_air_takes_the_daily_air_and_leaves_the_rest_null(pool: PgPool) {
    let history = week(pool).await;
    assert_eq!(
        row(&history, &day(14)),
        &json!({
            "date": day(14),
            "air_mean_f": 61.0,
            "air_min_f": 52.0,
            "air_max_f": null,
            "rh_mean_pct": null,
            "rain_in": null,
            "hours_observed": null,
            "soil_5cm_f": null,
            "gdd50": 11.0,
        })
    );
}

// @spec MCP-HIST-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_result_totals_rain_and_gdd_and_explains_partial_days(pool: PgPool) {
    let history = week(pool).await;
    assert_eq!(history["rain_total_in"], 0.96);
    assert_eq!(history["gdd50_total"], 66.0);
    let note = history["note"].as_str().unwrap();
    assert!(
        note.contains("A day with fewer than 24 hours observed is partial"),
        "{note}"
    );
}

// @spec MCP-HIST-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_no_location_the_tool_says_so_and_how_to_fix_it(pool: PgPool) {
    lawn(&pool).await;
    let state = lab_state(pool).await;
    let reply = call_tool(&state, "weather_history", json!({})).await;
    assert!(reply.is_error);
    assert_eq!(
        reply.text,
        format!(
            "Data source unavailable: TurfOps doesn't know where your lawn is yet, so \
             weather, timing and disease information are unavailable. Set the location in \
             Setup. {}",
            crate::mcp::tools::MISSING_DATA_HINT
        )
    );
}
