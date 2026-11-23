//! Fitting a depth's model on its calibration stations and writing the estimate series
//! it gives to the weather record.

use super::calibration::{self, Calibrator, Decision};
use super::estimator::{grid_key, station_key, Scene, SoilEstimator};
use super::model::{self, DailyAir, Features, Model};
use super::scores::Calibration;
use crate::db::soil_estimate_queries;
use crate::db::weather_queries::{days, rows};
use crate::error::Result;
use crate::logic::weather_record::resolve::COVERING_HOURS;
use crate::models::weather_record::{DailyRow, Input, Station};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A model is refitted when it is this old.
// @spec WX-EST-CAL-005
const REFIT_EVERY: Duration = Duration::days(30);
/// A station a model was fitted on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FitStation {
    pub station_ref: i64,
    pub name: String,
    pub distance_miles: f64,
}

/// A depth's model with what it rests on.
// @spec WX-EST-CAL-006
#[derive(Debug, Clone, PartialEq)]
pub struct StoredFit {
    pub depth_cm: i32,
    pub calibration: Calibration,
    pub model: Model,
    pub stations: Vec<FitStation>,
    /// Days of training data, over all the stations.
    pub training_days: i32,
    /// Root-mean-square daily error of the fit, °F.
    pub rmse_f: f64,
    pub fitted_at: DateTime<Utc>,
}

pub(super) fn depth_cm(depth: Input) -> i32 {
    match depth {
        Input::SoilTemp5 => 5,
        _ => 10,
    }
}

fn soil_of(row: &DailyRow, depth: Input) -> Option<f64> {
    match depth {
        Input::SoilTemp5 => row.soil_temp_5_f,
        _ => row.soil_temp_10_f,
    }
}

/// A grid's daily air: half the sum of its maximum and minimum.
pub(super) fn air_of(rows: &[DailyRow]) -> DailyAir {
    rows.iter()
        .filter_map(|row| Some((row.day, (row.air_max_f? + row.air_min_f?) / 2.0)))
        .collect()
}

impl SoilEstimator {
    /// Brings the estimate up to date: decides what each depth is fitted on, fits where
    /// a fit is due, and writes each fitted depth's series to the record.
    // @spec WX-EST-MODEL-003, WX-EST-MODEL-004, WX-EST-CAL-005, WX-EST-FAIL-001
    pub async fn run(
        &self,
        today: NaiveDate,
        now: DateTime<Utc>,
    ) -> Result<Vec<(Input, Decision)>> {
        let decisions = self.prepare(today).await?;
        let Some(scene) = self.scene().await? else {
            return Ok(decisions);
        };
        let mut fits: Vec<StoredFit> = soil_estimate_queries::all(&self.pool).await?;
        let mut series: BTreeMap<NaiveDate, DailyRow> = BTreeMap::new();
        let mut reach: Option<(NaiveDate, NaiveDate)> = None;
        for (depth, decision) in &decisions {
            let Decision::Fit(calibration, stations) = decision else {
                continue;
            };
            let chosen = (*calibration, stations.as_slice());
            self.refresh(&scene, *depth, chosen, &mut fits, today, now)
                .await?;
            let Some(fit) = fits.iter().find(|fit| fit.depth_cm == depth_cm(*depth)) else {
                continue;
            };
            let Some((air, span)) = self.driving_air(&scene, chosen, today).await? else {
                continue;
            };
            // The series is complete only where every depth's air is.
            reach = Some(reach.map_or(span, |held| (held.0.max(span.0), held.1.min(span.1))));
            for (day, value) in fit.model.series(&air) {
                let row = series.entry(day).or_insert_with(|| DailyRow {
                    day,
                    ..Default::default()
                });
                match depth {
                    Input::SoilTemp5 => row.soil_temp_5_f = Some(value),
                    _ => row.soil_temp_10_f = Some(value),
                }
            }
        }
        self.write(&series, reach, now).await?;
        Ok(decisions)
    }

    /// Fits a depth again when a fit is due, replacing the one held. A fit that cannot
    /// be made leaves the one before it standing.
    // @spec WX-EST-CAL-005, WX-EST-FAIL-003
    async fn refresh(
        &self,
        scene: &Scene,
        depth: Input,
        chosen: (Calibration, &[Calibrator]),
        fits: &mut Vec<StoredFit>,
        today: NaiveDate,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let held = fits.iter().find(|fit| fit.depth_cm == depth_cm(depth));
        if !self.refit_due(held, chosen.0, chosen.1, now) {
            return Ok(());
        }
        match self.fit(scene, depth, chosen, today, now).await? {
            Some(fit) => {
                tracing::info!(
                    depth_cm = fit.depth_cm,
                    fitted_on = ?fit.calibration,
                    stations = fit.stations.len(),
                    days = fit.training_days,
                    rmse_f = format!("{:.2}", fit.rmse_f),
                    "Soil estimate fitted"
                );
                soil_estimate_queries::store(&self.pool, &fit).await?;
                fits.retain(|held| held.depth_cm != fit.depth_cm);
                fits.push(fit);
            }
            None => tracing::warn!(depth = depth_cm(depth), "soil estimate refit failed"),
        }
        Ok(())
    }

    /// Whether a depth's model should be fitted now: it has none, it is a month old, or
    /// what it was fitted on has changed.
    // @spec WX-EST-CAL-005
    fn refit_due(
        &self,
        held: Option<&StoredFit>,
        calibration: Calibration,
        stations: &[Calibrator],
        now: DateTime<Utc>,
    ) -> bool {
        let Some(held) = held else {
            return true;
        };
        let fitted_on: Vec<i64> = held.stations.iter().map(|s| s.station_ref).collect();
        let chosen: Vec<i64> = stations.iter().map(|s| s.station_ref).collect();
        held.calibration != calibration
            || fitted_on != chosen
            || now - held.fitted_at >= REFIT_EVERY
    }

    /// Fits a depth on its stations: each station's soil paired with the grid's air at
    /// that station's point. `None` when the data gives no unique fit.
    // @spec WX-EST-CAL-003, WX-EST-CAL-004, WX-EST-CAL-006
    async fn fit(
        &self,
        scene: &Scene,
        depth: Input,
        (calibration, stations): (Calibration, &[Calibrator]),
        today: NaiveDate,
        now: DateTime<Utc>,
    ) -> Result<Option<StoredFit>> {
        let from = calibration::train_from();
        let mut training: Vec<(Features, f64)> = Vec::new();
        let mut fitted_on = Vec::new();
        for calibrator in stations {
            let Some(station) = scene.station(calibrator.station_ref) else {
                continue;
            };
            let Some(grid) = scene.source(&grid_key(station)) else {
                continue;
            };
            let air =
                air_of(&rows::daily(&self.pool, grid.id, from - Duration::days(30), today).await?);
            let soil = self
                .soil_of_station(scene, station, depth, calibration, today)
                .await?;
            training.extend(model::training_rows(&air, &soil, from));
            fitted_on.push(FitStation {
                station_ref: calibrator.station_ref,
                name: format!("{}, {}", station.name, station.state),
                distance_miles: calibrator.distance_miles,
            });
        }
        let Some(model) = model::fit(&training) else {
            return Ok(None);
        };
        let squared: f64 = training
            .iter()
            .map(|(inputs, soil)| (model.estimate(inputs) - soil).powi(2))
            .sum();
        Ok(Some(StoredFit {
            depth_cm: depth_cm(depth),
            calibration,
            rmse_f: (squared / training.len() as f64).sqrt(),
            training_days: training.len() as i32,
            model,
            stations: fitted_on,
            fitted_at: now,
        }))
    }

    /// A calibration station's daily soil at `depth`: its daily values, or for the basis
    /// station fitted on alone, the mean of each covered day's hours.
    async fn soil_of_station(
        &self,
        scene: &Scene,
        station: &Station,
        depth: Input,
        calibration: Calibration,
        today: NaiveDate,
    ) -> Result<BTreeMap<NaiveDate, f64>> {
        if calibration == Calibration::StationAlone {
            return self.basis_soil(scene, depth, today).await;
        }
        let Some(source) = scene.source(&station_key(station)) else {
            return Ok(BTreeMap::new());
        };
        let held = rows::daily(&self.pool, source.id, calibration::train_from(), today).await?;
        Ok(held
            .iter()
            .filter_map(|row| Some((row.day, soil_of(row, depth)?)))
            .collect())
    }

    /// The basis station's daily soil at `depth`: the mean of each covered day's hours.
    async fn basis_soil(
        &self,
        scene: &Scene,
        depth: Input,
        today: NaiveDate,
    ) -> Result<BTreeMap<NaiveDate, f64>> {
        let Some(source) = self.basis(depth).await? else {
            return Ok(BTreeMap::new());
        };
        let from = calibration::train_from();
        let held = days::local_days(&self.pool, source.id, scene.zone, from, today).await?;
        let at = usize::from(depth == Input::SoilTemp10);
        Ok(held
            .iter()
            .filter(|day| day.soil_temp[at].hours >= COVERING_HOURS)
            .filter_map(|day| Some((day.day, day.soil_temp[at].mean?)))
            .collect())
    }
}
