//! The 10 cm outlook's choice of method: fitted to the lawn's measured soil where it has
//! enough, otherwise the soil estimate carried into the forecast.

use super::{build_forecast, continuous_air, predict_threshold_crossings, MIN_TRAINING_PAIRS};
use crate::logic::soil_estimate::anchor::{forward, Days};
use crate::logic::soil_estimate::model::{DailyAir, Model};
use crate::models::soil_temp_prediction::*;
use crate::models::weather_basis::{Basis, BasisKind};
use chrono::{Duration, NaiveDate, Utc};

/// The soil estimate's 10 cm model and what it rests on.
#[derive(Debug, Clone)]
pub struct EstimateModel {
    pub model: Model,
    pub info: EstimateInfo,
}

/// Everything one outlook reads.
pub struct OutlookInputs<'a> {
    /// `(day, air mean, 10 cm soil, soil measured)` for the last 30 days.
    pub pairs: &'a [(NaiveDate, f64, f64, bool)],
    /// The record's daily air mean, far enough back for the estimate's trailing means.
    pub recorded_air: &'a [(NaiveDate, f64)],
    /// `(high + low) / 2` of the first 10 full forecast days.
    pub forecast_air: &'a [(NaiveDate, f64)],
    /// The record's daily 10 cm soil, `(day, value, measured)`, ascending.
    pub soil_days: &'a [(NaiveDate, f64, bool)],
    /// The live 10 cm reading and its basis.
    pub current: Option<(f64, Basis)>,
    /// The soil estimate's 10 cm model, when one has been fitted.
    pub estimate: Option<&'a EstimateModel>,
    /// Whether the estimate may date a threshold at 10 cm in a direction.
    pub datable: &'a dyn Fn(f64, CrossingDirection) -> bool,
}

/// The outlook, or `None` when neither method yields a prediction.
// @spec WX-OUTLOOK-METHOD-001, WX-OUTLOOK-METHOD-002, WX-OUTLOOK-METHOD-003
// @spec WX-OUTLOOK-METHOD-004, WX-OUTLOOK-MODEL-004, WX-OUTLOOK-MODEL-005, WX-OUTLOOK-MODEL-006
pub fn build_outlook(inputs: &OutlookInputs, today: NaiveDate) -> Option<SoilTempForecast> {
    let basis = inputs.current.as_ref().map(|(_, basis)| basis.clone());
    let measured_now = basis
        .as_ref()
        .is_some_and(|basis| basis.kind != BasisKind::Estimate);
    let measured: Vec<(NaiveDate, f64, f64)> = inputs
        .pairs
        .iter()
        .filter(|(_, _, _, measured)| *measured)
        .map(|(day, air, soil, _)| (*day, *air, *soil))
        .collect();
    if measured_now && measured.len() >= MIN_TRAINING_PAIRS {
        let current = inputs.current.as_ref().map(|(value, _)| *value)?;
        if let Some(mut fitted) = build_forecast(&measured, inputs.forecast_air, current, today) {
            fitted.basis = basis;
            return Some(fitted);
        }
    }
    estimated(inputs, basis, today)
}

/// The soil estimate's forward series over the recorded-then-forecast air, anchored where
/// it follows measured days, through the last forecast day.
fn estimated(
    inputs: &OutlookInputs,
    basis: Option<Basis>,
    today: NaiveDate,
) -> Option<SoilTempForecast> {
    let estimate = inputs.estimate?;
    let last_forecast_day = inputs.forecast_air.last()?.0;
    let air: DailyAir = continuous_air(inputs.recorded_air, inputs.forecast_air)
        .into_iter()
        .collect();
    let measured: Days = inputs
        .soil_days
        .iter()
        .filter(|(_, _, measured)| *measured)
        .map(|(day, value, _)| (*day, *value))
        .collect();
    let ahead = forward(&estimate.model, &air, &measured, today - Duration::days(1));
    let predictions: Vec<SoilTempPrediction> = ahead
        .into_iter()
        .filter(|(day, _)| *day <= last_forecast_day)
        .map(|(day, value)| SoilTempPrediction {
            date: day,
            predicted_soil_temp_f: (value * 10.0).round() / 10.0,
            confidence: if (day - today).num_days() <= 3 {
                PredictionConfidence::Medium
            } else {
                PredictionConfidence::Low
            },
            air_temp_used_f: air.get(&day).copied().unwrap_or_default(),
            source_description: "Soil estimate".into(),
        })
        .collect();
    let first = predictions.first()?.predicted_soil_temp_f;
    let current = inputs
        .current
        .as_ref()
        .map(|(value, _)| *value)
        .or_else(|| inputs.soil_days.last().map(|(_, value, _)| *value))
        .unwrap_or(first);
    let after_today: Vec<SoilTempPrediction> = predictions
        .iter()
        .filter(|p| p.date > today)
        .cloned()
        .collect();
    let threshold_crossings = predict_threshold_crossings(current, &after_today, today)
        .into_iter()
        .filter(|c| (inputs.datable)(c.threshold_temp_f, c.direction))
        .collect();
    Some(SoilTempForecast {
        predictions,
        threshold_crossings,
        method: OutlookMethod::Estimated,
        basis,
        model_info: None,
        estimate_info: Some(estimate.info.clone()),
        generated_at: Utc::now(),
    })
}
