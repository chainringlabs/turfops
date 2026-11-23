//! Where the estimate's series comes from and where it goes: the daily air that drives
//! it, and the record source it is written to.

use super::calibration::Calibrator;
use super::estimator::{grid_key, Scene, SoilEstimator};
use super::fit::air_of;
use super::model::DailyAir;
use super::scores::Calibration;
use crate::db::weather_queries::{fetch_state, plan, rows};
use crate::error::Result;
use crate::models::weather_record::{DailyRow, Input, Source, SourceKind};
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::BTreeMap;

/// The name the estimate's span is kept under in the record.
const PRODUCT: &str = "soil_estimate";

impl SoilEstimator {
    /// The daily air the series is computed from, and the span it is complete over:
    /// the grid at the lawn's point for a model fitted on the five nearest, the grid at
    /// the basis station's point for one fitted on that station alone.
    // @spec WX-EST-MODEL-006
    pub(super) async fn driving_air(
        &self,
        scene: &Scene,
        (calibration, stations): (Calibration, &[Calibrator]),
        today: NaiveDate,
    ) -> Result<Option<(DailyAir, (NaiveDate, NaiveDate))>> {
        let grids: Vec<Source> = match calibration {
            Calibration::StationAlone => stations
                .first()
                .and_then(|calibrator| scene.station(calibrator.station_ref))
                .and_then(|station| scene.source(&grid_key(station)))
                .cloned()
                .into_iter()
                .collect(),
            Calibration::NearestFive => plan::read(&self.pool)
                .await?
                .into_iter()
                .filter(|placed| placed.input == Input::DailyAir)
                .map(|placed| placed.source)
                .filter(|source| source.kind == SourceKind::Grid)
                .collect(),
        };
        // The first grid's span is the estimate's; a later grid fills days it lacks.
        let Some(span) = grids.first().and_then(|grid| scene.span_held(grid)) else {
            return Ok(None);
        };
        let mut air = DailyAir::new();
        for grid in &grids {
            let rows = rows::daily(&self.pool, grid.id, span.0, today).await?;
            for (day, value) in air_of(&rows) {
                air.entry(day).or_insert(value);
            }
        }
        Ok(Some((air, span)))
    }

    /// Writes the series to the record's estimate source and marks the span it covers.
    // @spec WX-EST-MODEL-003, WX-EST-MODEL-005
    pub(super) async fn write(
        &self,
        series: &BTreeMap<NaiveDate, DailyRow>,
        reach: Option<(NaiveDate, NaiveDate)>,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let estimate = plan::read(&self.pool)
            .await?
            .into_iter()
            .map(|placed| placed.source)
            .find(|source| source.kind == SourceKind::Estimate);
        let (Some(estimate), Some(span)) = (estimate, reach) else {
            return Ok(());
        };
        if series.is_empty() {
            return Ok(());
        }
        let rows: Vec<DailyRow> = series.values().cloned().collect();
        fetch_state::want(&self.pool, estimate.id, PRODUCT, span.0).await?;
        fetch_state::daily_fetched(&self.pool, &estimate, PRODUCT, &rows, span, now).await
    }

    /// The station the plan holds for soil at `depth`, if any.
    pub(super) async fn basis(&self, depth: Input) -> Result<Option<Source>> {
        Ok(plan::read(&self.pool)
            .await?
            .into_iter()
            .find(|placed| placed.input == depth && placed.source.kind == SourceKind::Station)
            .map(|placed| placed.source))
    }
}
