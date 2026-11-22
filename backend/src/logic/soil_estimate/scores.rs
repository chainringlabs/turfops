//! The scoring tool's data file, embedded at build time, and `datable`.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// What a model was fitted on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Calibration {
    /// The five nearest stations, pooled; no soil from the target.
    NearestFive,
    /// The basis station's own record.
    StationAlone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Rising,
    Falling,
}

/// One crossing scored under one calibration. Errors and spreads are absolute days.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossingScore {
    pub depth_cm: u32,
    pub threshold_f: f64,
    pub direction: Direction,
    pub calibration: Calibration,
    pub median_days: f64,
    pub p80_days: f64,
    /// Mean signed error; positive = the estimate dates the crossing late.
    pub bias_days: f64,
    pub spread_median_days: f64,
    pub spread_p80_days: f64,
    pub samples: u32,
}

/// Root-mean-square daily error of the series at one depth under one calibration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyError {
    pub depth_cm: u32,
    pub calibration: Calibration,
    pub rmse_f: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreFile {
    pub generated_on: NaiveDate,
    pub stations: u32,
    pub first_year: i32,
    pub last_year: i32,
    pub daily_error: Vec<DailyError>,
    pub crossings: Vec<CrossingScore>,
}

/// The scored error of a crossing the estimate may date.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Accuracy {
    pub median_days: f64,
    pub p80_days: f64,
}

impl ScoreFile {
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// The scored accuracy when the estimate may date this crossing under this
    /// calibration: it was scored, and its median error beats the spread between years.
    // @spec WX-EST-DATE-001, WX-EST-DATE-002, WX-EST-DATE-003
    pub fn datable(
        &self,
        depth_cm: u32,
        threshold_f: f64,
        direction: Direction,
        calibration: Calibration,
    ) -> Option<Accuracy> {
        let score = self.crossings.iter().find(|s| {
            s.depth_cm == depth_cm
                && s.threshold_f == threshold_f
                && s.direction == direction
                && s.calibration == calibration
        })?;
        (score.median_days < score.spread_median_days).then_some(Accuracy {
            median_days: score.median_days,
            p80_days: score.p80_days,
        })
    }
}

/// The data file written by the scoring tool and compiled into the binary.
// @spec WX-EST-SCORE-003
pub fn embedded() -> &'static ScoreFile {
    static FILE: OnceLock<ScoreFile> = OnceLock::new();
    FILE.get_or_init(|| {
        ScoreFile::parse(include_str!("scores.json")).expect("embedded soil-estimate scores")
    })
}
