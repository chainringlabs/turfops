use crate::db::soil_estimate_queries;
use crate::error::{Source, TurfOpsError};
use crate::logic::data_sync::record_asked;
use crate::logic::soil_estimate::scores::{self, Direction};
use crate::logic::soil_temp_prediction::{self, EstimateModel, OutlookInputs};
use crate::logic::weather_record::record::WeatherRecord;
use crate::models::soil_temp_prediction::{CrossingDirection, EstimateInfo, SoilTempForecast};
use crate::models::weather_basis::{BasisKind, BasisSpan};
use crate::models::weather_record::Input;
use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use chrono::{Duration, NaiveDate};

/// Days of record history the soil estimate's trailing means need before the outlook.
const AIR_HISTORY_DAYS: i64 = 60;
/// Days of record the fitted method trains on.
const TRAINING_DAYS: i64 = 30;
/// The outlook's furthest reach in full forecast days.
const OUTLOOK_DAYS: usize = 10;

// @spec WX-OUTLOOK-API-001, WX-OUTLOOK-API-002, WX-OUTLOOK-API-003, WX-OUTLOOK-MODEL-004
// @spec WX-OUTLOOK-MODEL-005, WX-OUTLOOK-MODEL-008
/// GET /api/v1/soil-temp-forecast: the 10 cm outlook from the weather record, fitted to
/// the lawn's measured soil or carried forward by the soil estimate.
pub async fn get_soil_temp_forecast(
    State(state): State<AppState>,
) -> Result<Json<SoilTempForecast>, TurfOpsError> {
    let summary = {
        let mut service = state.sync_service.write().await;
        service.get_or_refresh().await?
    };
    let asked = record_asked(&state.pool, &state.clock.get()).await?;
    let today = asked.at.today;
    let record = WeatherRecord::new(state.pool.clone());
    let pairs = record
        .fetch_daily_paired_averages(today - Duration::days(TRAINING_DAYS), today, asked)
        .await?;
    let soil = record
        .fetch_daily_soil_temp_averages(today - Duration::days(AIR_HISTORY_DAYS), today, asked)
        .await?;
    let climate = record
        .fetch_climate_record(today - Duration::days(AIR_HISTORY_DAYS), asked)
        .await?;

    let estimated_days = estimated_soil_days(&soil.basis);
    let measured = |day: NaiveDate| {
        !estimated_days
            .iter()
            .any(|(from, to)| *from <= day && day <= *to)
    };
    let pairs: Vec<(NaiveDate, f64, f64, bool)> = pairs
        .rows
        .iter()
        .map(|(day, air, soil)| (*day, *air, *soil, measured(*day)))
        .collect();
    let soil_days: Vec<(NaiveDate, f64, bool)> = soil
        .rows
        .iter()
        .map(|row| (row.date, row.avg_temp_f, measured(row.date)))
        .collect();
    let recorded_air: Vec<(NaiveDate, f64)> = climate
        .rows
        .iter()
        .filter_map(|day| Some((day.date, day.air_avg_f?)))
        .collect();

    // The record's outages (no location, history loading) come first; then the forecast.
    let forecast_air: Vec<(NaiveDate, f64)> = summary
        .forecast
        .as_ref()
        .map(|f| {
            // Partial first/last days would enter as a full-day mean.
            f.full_days()
                .take(OUTLOOK_DAYS)
                .map(|d| (d.date, (d.high_temp_f + d.low_temp_f) / 2.0))
                .collect()
        })
        .unwrap_or_default();
    if forecast_air.is_empty() {
        return Err(TurfOpsError::unavailable(
            Source::Forecast,
            "no full forecast days to drive the soil outlook",
        ));
    }

    let current = summary.current.as_ref().and_then(|c| c.soil_temp_10_f).zip(
        summary
            .current_bases
            .iter()
            .find(|b| b.input == Input::SoilTemp10.as_str())
            .and_then(|b| b.basis.clone()),
    );

    let fit = soil_estimate_queries::all(&state.pool)
        .await?
        .into_iter()
        .find(|fit| fit.depth_cm == 10);
    let estimate = fit.as_ref().map(|fit| EstimateModel {
        model: fit.model.clone(),
        info: EstimateInfo {
            stations: fit.stations.iter().map(|s| s.name.clone()).collect(),
            training_days: fit.training_days,
            daily_error_f: fit.rmse_f,
        },
    });
    let calibration = fit.as_ref().map(|fit| fit.calibration);
    let datable = |threshold: f64, direction: CrossingDirection| {
        let direction = match direction {
            CrossingDirection::Rising => Direction::Rising,
            CrossingDirection::Falling => Direction::Falling,
        };
        calibration.is_some_and(|calibration| {
            scores::embedded()
                .datable(10, threshold, direction, calibration)
                .is_some()
        })
    };

    let outlook = soil_temp_prediction::build_outlook(
        &OutlookInputs {
            pairs: &pairs,
            recorded_air: &recorded_air,
            forecast_air: &forecast_air,
            soil_days: &soil_days,
            current,
            estimate: estimate.as_ref(),
            datable: &datable,
        },
        today,
    )
    .ok_or_else(|| TurfOpsError::not_enough_data(Source::SoilOutlook))?;
    Ok(Json(outlook))
}

/// The days a 10 cm series rests on the soil estimate.
fn estimated_soil_days(spans: &[BasisSpan]) -> Vec<(NaiveDate, NaiveDate)> {
    spans
        .iter()
        .filter(|span| span.input == Input::SoilTemp10 && span.basis.kind == BasisKind::Estimate)
        .map(|span| (span.from, span.to))
        .collect()
}

#[cfg(all(test, feature = "db-tests"))]
mod db_tests;
