use crate::error::TurfOpsError;
use crate::logic::data_sync::record_asked;
use crate::logic::gdd;
use crate::logic::weather_record::daily::period_basis;
use crate::logic::weather_record::record::WeatherRecord;
use crate::models::GddSummary;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::Json;
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GddQuery {
    pub year: Option<i32>,
}

// @spec RULE-GDD-001, RULE-GDD-006, RULE-GDD-007, WX-VIEW-BASIS-001
/// GET /api/v1/gdd?year=2026: the year's running GDD from the weather record's daily
/// `gdd50`, through today for the current year (the lawn's local date).
pub async fn get_gdd(
    State(state): State<AppState>,
    Query(params): Query<GddQuery>,
) -> Result<Json<GddSummary>, TurfOpsError> {
    let asked = record_asked(&state.pool, &state.clock.get()).await?;
    let today = asked.at.today;
    let year = params.year.unwrap_or_else(|| today.year());
    let jan1 = NaiveDate::from_ymd_opt(year, 1, 1)
        .ok_or_else(|| TurfOpsError::InvalidData(format!("Invalid year: {}", year)))?;

    // For the current year, stop at today; otherwise the full calendar year.
    let end_date = if year == today.year() {
        today
    } else {
        NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or(today)
    };

    // A record that cannot answer is an error, never a zero total.
    let (records, basis) = if jan1 <= end_date {
        let series = WeatherRecord::new(state.pool.clone())
            .fetch_daily_gdd(jan1, end_date, asked)
            .await?;
        (
            gdd::accumulate_daily_gdd(&series.rows),
            period_basis(&series.basis),
        )
    } else {
        (Vec::new(), None)
    };

    Ok(Json(GddSummary {
        basis,
        ..gdd::build_gdd_summary(year, &records)
    }))
}

#[cfg(all(test, feature = "db-tests"))]
mod db_tests;
