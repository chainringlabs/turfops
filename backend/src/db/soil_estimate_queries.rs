//! The soil estimate's stored fits: one per depth, replaced on a refit.

use crate::error::{Result, TurfOpsError};
use crate::logic::soil_estimate::fit::{FitStation, StoredFit};
use crate::logic::soil_estimate::model::{Model, TERMS};
use crate::logic::soil_estimate::scores::Calibration;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

fn name_of(calibration: Calibration) -> &'static str {
    match calibration {
        Calibration::NearestFive => "NearestFive",
        Calibration::StationAlone => "StationAlone",
    }
}

/// Stores `fit` as its depth's fit, in place of any before it.
// @spec WX-EST-CAL-006
pub async fn store(pool: &PgPool, fit: &StoredFit) -> Result<()> {
    let stations = serde_json::to_string(&fit.stations)
        .map_err(|e| TurfOpsError::Internal(format!("fit stations: {e}")))?;
    sqlx::query(
        "INSERT INTO soil_estimate_fit \
             (depth_cm, calibration, coefficients, stations, training_days, rmse_f, fitted_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (depth_cm) DO UPDATE SET \
             calibration = EXCLUDED.calibration, coefficients = EXCLUDED.coefficients, \
             stations = EXCLUDED.stations, training_days = EXCLUDED.training_days, \
             rmse_f = EXCLUDED.rmse_f, fitted_at = EXCLUDED.fitted_at",
    )
    .bind(fit.depth_cm)
    .bind(name_of(fit.calibration))
    .bind(fit.model.coefficients.to_vec())
    .bind(stations)
    .bind(fit.training_days)
    .bind(fit.rmse_f)
    .bind(fit.fitted_at)
    .execute(pool)
    .await?;
    Ok(())
}

type Row = (i32, String, Vec<f64>, String, i32, f64, DateTime<Utc>);

/// The fits held, by depth.
pub async fn all(pool: &PgPool) -> Result<Vec<StoredFit>> {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT depth_cm, calibration, coefficients, stations, training_days, rmse_f, fitted_at \
         FROM soil_estimate_fit ORDER BY depth_cm",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(stored).collect()
}

fn stored(row: Row) -> Result<StoredFit> {
    let (depth_cm, calibration, coefficients, stations, training_days, rmse_f, fitted_at) = row;
    let broken = |what: &str| TurfOpsError::Internal(format!("soil estimate fit: {what}"));
    let coefficients: [f64; TERMS] = coefficients
        .try_into()
        .map_err(|_| broken("coefficients"))?;
    let stations: Vec<FitStation> =
        serde_json::from_str(&stations).map_err(|_| broken("stations"))?;
    Ok(StoredFit {
        depth_cm,
        calibration: match calibration.as_str() {
            "NearestFive" => Calibration::NearestFive,
            "StationAlone" => Calibration::StationAlone,
            _ => return Err(broken("calibration")),
        },
        model: Model { coefficients },
        stations,
        training_days,
        rmse_f,
        fitted_at,
    })
}
