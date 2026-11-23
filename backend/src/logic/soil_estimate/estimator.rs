//! The soil estimate at work for one lawn: deciding what each depth's model is fitted
//! on, and having the record hold the stations and grid points that needs.

use super::calibration::{self, Calibrator, Decision};
use crate::db::weather_queries::{catalog, days, fetch_state, plan, rows, sources};
use crate::error::{Result, TurfOpsError};
use crate::models::weather_record::{
    FetchState, Input, NewSource, Source, SourceKind, SourceRole, Station,
};
use chrono::{Datelike, NaiveDate};
use chrono_tz::Tz;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};

/// The depths the estimate is made at.
pub const DEPTHS: [Input; 2] = [Input::SoilTemp5, Input::SoilTemp10];

pub struct SoilEstimator {
    pub(super) pool: PgPool,
}

/// What a decision rests on, read once.
pub(super) struct Scene {
    latitude: f64,
    longitude: f64,
    pub(super) zone: Tz,
    catalog: Vec<(i64, Station)>,
    sources: Vec<Source>,
    states: HashMap<i64, FetchState>,
}

/// The key of the source holding a calibration station's daily values.
pub(super) fn station_key(station: &Station) -> String {
    format!(
        "calibration:{}:{}",
        station.network.as_str(),
        station.station_id
    )
}

/// The key of the source holding the grid's daily air at a station's point.
pub(super) fn grid_key(station: &Station) -> String {
    format!(
        "calibration:grid:prism:{:.4},{:.4}",
        station.latitude, station.longitude
    )
}

impl SoilEstimator {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Decides what each depth's model is fitted on, and brings the record's
    /// calibration sources in line: the ones the decisions need are registered with
    /// the span they are wanted for, and the ones they no longer need are removed.
    /// A lawn with no location has no decisions.
    // @spec WX-EST-CAL-007, WX-EST-FAIL-001
    pub async fn prepare(&self, today: NaiveDate) -> Result<Vec<(Input, Decision)>> {
        let Some(scene) = self.scene().await? else {
            return Ok(Vec::new());
        };
        let mut decisions = Vec::new();
        for depth in DEPTHS {
            decisions.push((depth, self.decide(&scene, depth, today).await?));
        }
        self.hold(&scene, &decisions, today).await?;
        Ok(decisions)
    }

    pub(super) async fn scene(&self) -> Result<Option<Scene>> {
        let profile = crate::db::queries::get_default_lawn_profile(&self.pool).await?;
        let Some(location) = profile.and_then(|profile| profile.location) else {
            return Ok(None);
        };
        let zone = location
            .timezone
            .parse::<Tz>()
            .map_err(|_| TurfOpsError::Internal("the lawn's timezone is not a zone".into()))?;
        let states = fetch_state::all(&self.pool).await?;
        Ok(Some(Scene {
            latitude: location.latitude,
            longitude: location.longitude,
            zone,
            catalog: catalog::stations(&self.pool).await?,
            sources: sources::all(&self.pool).await?,
            states: states.into_iter().map(|s| (s.source_id, s)).collect(),
        }))
    }

    async fn decide(&self, scene: &Scene, depth: Input, today: NaiveDate) -> Result<Decision> {
        let basis = plan::read(&self.pool)
            .await?
            .into_iter()
            .find(|placed| placed.input == depth && placed.source.kind == SourceKind::Station)
            .map(|placed| placed.source);
        let mut soil_days = HashMap::new();
        let basis = match basis {
            Some(source) => {
                let station_ref = source.station_ref.unwrap_or_default();
                if scene.holds_history(&source) {
                    let counted = self.basis_days(scene, &source, depth, today).await?;
                    soil_days.insert(station_ref, counted);
                }
                Some(Calibrator {
                    station_ref,
                    distance_miles: source.distance_miles.unwrap_or_default(),
                })
            }
            None => None,
        };
        let candidates = calibration::candidates(
            scene.latitude,
            scene.longitude,
            depth,
            today.year(),
            &scene.catalog,
        );
        for candidate in &candidates {
            let held = scene.station(candidate.station_ref).and_then(|station| {
                scene
                    .source(&station_key(station))
                    .filter(|s| scene.holds_history(s))
            });
            // The basis station's own count, when it has one, stands.
            if let (Some(source), false) = (held, soil_days.contains_key(&candidate.station_ref)) {
                let counted = self.calibration_days(source, depth, today).await?;
                soil_days.insert(candidate.station_ref, counted);
            }
        }
        Ok(calibration::decide(basis, &candidates, &soil_days))
    }

    /// Days of soil at `depth` a basis station covers, from the first training day.
    async fn basis_days(
        &self,
        scene: &Scene,
        source: &Source,
        depth: Input,
        today: NaiveDate,
    ) -> Result<usize> {
        let from = calibration::train_from();
        let held = days::local_days(&self.pool, source.id, scene.zone, from, today).await?;
        let at = depth_index(depth);
        Ok(held
            .iter()
            .filter(|day| {
                day.soil_temp[at].hours >= crate::logic::weather_record::resolve::COVERING_HOURS
            })
            .count())
    }

    /// Days of soil at `depth` a calibration station holds, from the first training day.
    async fn calibration_days(
        &self,
        source: &Source,
        depth: Input,
        today: NaiveDate,
    ) -> Result<usize> {
        let held = rows::daily(&self.pool, source.id, calibration::train_from(), today).await?;
        Ok(held
            .iter()
            .filter(|row| match depth {
                Input::SoilTemp5 => row.soil_temp_5_f.is_some(),
                _ => row.soil_temp_10_f.is_some(),
            })
            .count())
    }

    /// Creates and registers the calibration sources the decisions need, and removes
    /// the ones they do not.
    // @spec WX-EST-CAL-003, WX-EST-CAL-007
    async fn hold(
        &self,
        scene: &Scene,
        decisions: &[(Input, Decision)],
        today: NaiveDate,
    ) -> Result<()> {
        let wanted = needed(scene, decisions);
        let keys: HashSet<&str> = wanted.iter().map(|source| source.key.as_str()).collect();
        // A station passed over for holding too little stays, nearer than the farthest
        // one chosen: its count is why it was passed over, and it may yet grow enough.
        let reach = wanted
            .iter()
            .filter_map(|source| source.distance_miles)
            .fold(0.0, f64::max);
        for source in &scene.sources {
            let passed_over = source.kind == SourceKind::Station
                && source.distance_miles.is_some_and(|miles| miles <= reach);
            let dropped = source.role == SourceRole::Calibration
                && !keys.contains(source.key.as_str())
                && !passed_over;
            if dropped {
                sources::delete(&self.pool, source.id).await?;
            }
        }
        for new in &wanted {
            let stored = sources::ensure(&self.pool, new).await?;
            let wanted_for = crate::logic::weather_record::due::wanted_for(&stored, today, today);
            if let Some((product, from)) = wanted_for {
                fetch_state::want(&self.pool, stored.id, product.as_str(), from).await?;
            }
        }
        Ok(())
    }
}

impl Scene {
    pub(super) fn station(&self, station_ref: i64) -> Option<&Station> {
        self.catalog
            .iter()
            .find(|(id, _)| *id == station_ref)
            .map(|(_, station)| station)
    }

    pub(super) fn source(&self, key: &str) -> Option<&Source> {
        self.sources.iter().find(|source| source.key == key)
    }

    /// The span a source holds, as far as it is wanted: from the first day wanted when
    /// its history is in, from the first day held until then.
    pub(super) fn span_held(&self, source: &Source) -> Option<(NaiveDate, NaiveDate)> {
        let state = self.states.get(&source.id)?;
        let (from, through) = state.held?;
        Some((from.max(state.wanted_from), through))
    }

    /// Whether a source holds everything it is wanted for, so its days can be counted.
    fn holds_history(&self, source: &Source) -> bool {
        self.states.get(&source.id).is_some_and(|state| {
            state
                .held
                .is_some_and(|(from, _)| from <= state.wanted_from)
        })
    }
}

/// The calibration sources a set of decisions needs: each chosen station's daily values
/// and the grid's daily air at its point.
// @spec WX-EST-CAL-003
fn needed(scene: &Scene, decisions: &[(Input, Decision)]) -> Vec<NewSource> {
    let mut wanted: Vec<NewSource> = Vec::new();
    for (_, decision) in decisions {
        let (alone, stations) = match decision {
            Decision::Fit(calibration, stations) => (
                *calibration == super::scores::Calibration::StationAlone,
                stations,
            ),
            // A lone station being waited for is the basis station.
            Decision::Wait(stations) => (stations.len() == 1, stations),
            Decision::NoModel => continue,
        };
        for calibrator in stations {
            let Some(station) = scene.station(calibrator.station_ref) else {
                continue;
            };
            // The basis station's soil is already held as its hourly rows.
            if !alone {
                wanted.push(calibration_source(station, calibrator, SourceKind::Station));
            }
            wanted.push(calibration_source(station, calibrator, SourceKind::Grid));
        }
    }
    wanted
}

fn depth_index(depth: Input) -> usize {
    match depth {
        Input::SoilTemp5 => 0,
        _ => 1,
    }
}

/// A calibration station's daily values, or the grid's daily air at its point.
fn calibration_source(station: &Station, calibrator: &Calibrator, kind: SourceKind) -> NewSource {
    let (key, name) = match kind {
        SourceKind::Grid => (
            grid_key(station),
            format!("PRISM daily grid at {}, {}", station.name, station.state),
        ),
        _ => (
            station_key(station),
            format!("{}, {} (USCRN)", station.name, station.state),
        ),
    };
    NewSource {
        key,
        kind,
        name,
        station_ref: (kind == SourceKind::Station).then_some(calibrator.station_ref),
        latitude: Some(station.latitude),
        longitude: Some(station.longitude),
        distance_miles: (kind == SourceKind::Station).then_some(calibrator.distance_miles),
        role: SourceRole::Calibration,
    }
}
