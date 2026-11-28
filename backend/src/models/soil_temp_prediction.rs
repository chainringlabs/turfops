use crate::models::weather_basis::Basis;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// Fitted linear regression model: soil_temp = slope * lagged_air_temp + intercept
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilTempModel {
    pub slope: f64,
    pub intercept: f64,
    pub lag_days: u32,
    pub r_squared: f64,
    pub training_window_days: u32,
    pub fitted_at: DateTime<Utc>,
}

/// A single day's predicted soil temperature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilTempPrediction {
    pub date: NaiveDate,
    pub predicted_soil_temp_f: f64,
    pub confidence: PredictionConfidence,
    pub air_temp_used_f: f64,
    pub source_description: String,
}

/// Confidence level for a prediction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PredictionConfidence {
    High,
    Medium,
    Low,
}

impl PredictionConfidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            PredictionConfidence::High => "High",
            PredictionConfidence::Medium => "Medium",
            PredictionConfidence::Low => "Low",
        }
    }
}

impl std::fmt::Display for PredictionConfidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Direction a threshold is being crossed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrossingDirection {
    Rising,
    Falling,
}

impl std::fmt::Display for CrossingDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CrossingDirection::Rising => write!(f, "Rising"),
            CrossingDirection::Falling => write!(f, "Falling"),
        }
    }
}

/// A predicted threshold crossing event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdPrediction {
    pub threshold_name: String,
    pub threshold_temp_f: f64,
    /// `None` when the estimate falls past the last forecast day (`beyond_forecast`).
    pub estimated_crossing_date: Option<NaiveDate>,
    pub days_until_crossing: i64,
    /// The crossing is extrapolated past the forecast, so it has no date.
    #[serde(default)]
    pub beyond_forecast: bool,
    pub confidence: PredictionConfidence,
    pub direction: CrossingDirection,
}

/// How the 10 cm outlook was made.
// @spec WX-OUTLOOK-METHOD-001, WX-OUTLOOK-METHOD-002
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutlookMethod {
    /// The lagged regression fitted to the lawn's measured 10 cm soil.
    Fitted,
    /// The soil estimate carried into the forecast.
    Estimated,
}

/// Full API response for soil temperature forecast
// @spec WX-OUTLOOK-API-001
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilTempForecast {
    pub predictions: Vec<SoilTempPrediction>,
    pub threshold_crossings: Vec<ThresholdPrediction>,
    pub method: OutlookMethod,
    /// What the soil the outlook starts from rests on.
    pub basis: Option<Basis>,
    /// The fit, on the fitted method.
    pub model_info: Option<SoilTempModelInfo>,
    /// The soil estimate behind it, on the estimated method.
    pub estimate_info: Option<EstimateInfo>,
    pub generated_at: DateTime<Utc>,
}

/// The soil estimate an estimated outlook rests on; no coefficients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstimateInfo {
    /// The stations the estimate was fitted on.
    pub stations: Vec<String>,
    pub training_days: i32,
    /// Its scored root-mean-square daily error, °F, from the scoring file; null when the
    /// file has no score for the fit's depth and calibration.
    pub daily_error_f: Option<f64>,
}

/// Model metadata for display (excludes internal coefficients)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilTempModelInfo {
    pub r_squared: f64,
    pub lag_days: u32,
    pub training_window_days: u32,
    pub quality: String,
}

// @spec WX-OUTLOOK-MODEL-006
impl From<&SoilTempModel> for SoilTempModelInfo {
    fn from(model: &SoilTempModel) -> Self {
        let quality = if model.r_squared >= 0.7 {
            "Good"
        } else if model.r_squared >= 0.5 {
            "Fair"
        } else {
            "Poor"
        }
        .to_string();

        Self {
            r_squared: model.r_squared,
            lag_days: model.lag_days,
            training_window_days: model.training_window_days,
            quality,
        }
    }
}
