//! Scores the estimate against a network of stations and assembles the data file.

use super::super::model::{fit, training_rows, DailyAir, Features, Model};
use super::super::scores::{Calibration, CrossingScore, DailyError, ScoreFile};
use super::crossings::{date_in_year, plan_crossings, timing_crossings, ScoredCrossing};
use super::inputs::StationSoil;
use super::stats::{median, spread, summarize};
use crate::geo::miles_between;
use chrono::{Datelike, NaiveDate};
use std::collections::BTreeMap;

/// A station needs this many days of soil at a depth to calibrate or be scored there.
pub const MIN_SOIL_DAYS: usize = 1000;
/// A year is scored when the measured series has at least this many days in it.
pub const MIN_DAYS_IN_YEAR: usize = 300;
/// Stations pooled in the nearest-neighbour calibration.
pub const NEIGHBOURS: usize = 5;

type Soil = BTreeMap<NaiveDate, f64>;

/// One station: its measured soil and the grid's daily air at its point.
#[derive(Debug, Clone)]
pub struct Station {
    pub name: String,
    pub soil: StationSoil,
    pub air: DailyAir,
}

/// A station eligible at one depth, with its training rows.
struct Member<'a> {
    station: &'a Station,
    soil: &'a Soil,
    rows: Vec<(Features, f64)>,
}

/// Signed day errors and between-year spread for one crossing under one calibration.
#[derive(Default)]
struct Tally {
    errors: Vec<f64>,
    spread: Vec<f64>,
}

/// Scores every crossing a consumer dates, at both depths and under both calibrations.
// @spec WX-EST-SCORE-001, WX-EST-SCORE-002
pub fn score(
    stations: &[Station],
    first_year: i32,
    last_year: i32,
    generated_on: NaiveDate,
) -> ScoreFile {
    let mut file = ScoreFile {
        generated_on,
        stations: 0,
        first_year,
        last_year,
        daily_error: Vec::new(),
        crossings: Vec::new(),
    };
    let from = NaiveDate::from_ymd_opt(first_year, 1, 1).expect("a valid first year");
    for (depth_cm, crossings) in [(5, timing_crossings()), (10, plan_crossings())] {
        let members = members(stations, depth_cm, from);
        file.stations = file.stations.max(members.len() as u32);
        for calibration in [Calibration::NearestFive, Calibration::StationAlone] {
            let depth = score_depth(&members, &crossings, calibration, first_year..=last_year);
            file.daily_error
                .extend(depth.rmse_f.map(|rmse_f| DailyError {
                    depth_cm,
                    calibration,
                    rmse_f: round(rmse_f),
                }));
            file.crossings
                .extend(crossing_scores(&crossings, &depth.tallies, calibration));
        }
    }
    file
}

fn soil_at(station: &Station, depth_cm: u32) -> &Soil {
    if depth_cm == 5 {
        &station.soil.soil_5cm_f
    } else {
        &station.soil.soil_10cm_f
    }
}

fn members(stations: &[Station], depth_cm: u32, from: NaiveDate) -> Vec<Member<'_>> {
    stations
        .iter()
        .map(|station| (station, soil_at(station, depth_cm)))
        .filter(|(_, soil)| soil.len() >= MIN_SOIL_DAYS)
        .map(|(station, soil)| Member {
            station,
            soil,
            rows: training_rows(&station.air, soil, from),
        })
        .collect()
}

struct DepthResult {
    tallies: Vec<Tally>,
    rmse_f: Option<f64>,
}

fn score_depth(
    members: &[Member<'_>],
    crossings: &[ScoredCrossing],
    calibration: Calibration,
    years: std::ops::RangeInclusive<i32>,
) -> DepthResult {
    let mut tallies: Vec<Tally> = crossings.iter().map(|_| Tally::default()).collect();
    let mut rmse = Vec::new();
    for (index, member) in members.iter().enumerate() {
        let Some(model) = calibrate(members, index, calibration) else {
            continue;
        };
        let predicted: Soil = model.series(&member.station.air).into_iter().collect();
        rmse.extend(daily_rmse(member.soil, &predicted));
        for (crossing, tally) in crossings.iter().zip(&mut tallies) {
            tally_station(crossing, member.soil, &predicted, years.clone(), tally);
        }
    }
    DepthResult {
        tallies,
        rmse_f: median(&rmse),
    }
}

/// The model for `members[target]`: its own rows, or the pooled rows of its nearest
/// neighbours, never its own.
// @spec WX-EST-SCORE-002
fn calibrate(members: &[Member<'_>], target: usize, calibration: Calibration) -> Option<Model> {
    let own = &members[target];
    if calibration == Calibration::StationAlone {
        return fit(&own.rows);
    }
    let here = &own.station.soil;
    let mut others: Vec<(f64, &Member<'_>)> = members
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != target)
        .map(|(_, other)| {
            let there = &other.station.soil;
            let miles = miles_between(
                here.latitude,
                here.longitude,
                there.latitude,
                there.longitude,
            );
            (miles, other)
        })
        .collect();
    others.sort_by(|a, b| a.0.total_cmp(&b.0));
    let pooled: Vec<(Features, f64)> = others
        .iter()
        .take(NEIGHBOURS)
        .flat_map(|(_, other)| other.rows.iter().copied())
        .collect();
    fit(&pooled)
}

fn daily_rmse(measured: &Soil, predicted: &Soil) -> Option<f64> {
    let squares: Vec<f64> = predicted
        .iter()
        .filter_map(|(day, estimate)| Some((estimate - measured.get(day)?).powi(2)))
        .collect();
    (!squares.is_empty()).then(|| (squares.iter().sum::<f64>() / squares.len() as f64).sqrt())
}

fn days_in(soil: &Soil, year: i32) -> usize {
    soil.keys().filter(|day| day.year() == year).count()
}

fn tally_station(
    crossing: &ScoredCrossing,
    measured: &Soil,
    predicted: &Soil,
    years: std::ops::RangeInclusive<i32>,
    tally: &mut Tally,
) {
    let mut measured_dates = Vec::new();
    for year in years {
        if days_in(measured, year) < MIN_DAYS_IN_YEAR {
            continue;
        }
        let Some(truth) = date_in_year(crossing, measured, year) else {
            continue;
        };
        measured_dates.push(f64::from(truth));
        if let Some(estimate) = date_in_year(crossing, predicted, year) {
            tally.errors.push(f64::from(estimate) - f64::from(truth));
        }
    }
    tally.spread.extend(spread(&measured_dates));
}

fn crossing_scores(
    crossings: &[ScoredCrossing],
    tallies: &[Tally],
    calibration: Calibration,
) -> Vec<CrossingScore> {
    crossings
        .iter()
        .zip(tallies)
        .filter_map(|(crossing, tally)| {
            let errors = summarize(&tally.errors)?;
            let between_years = summarize(&tally.spread)?;
            Some(CrossingScore {
                depth_cm: crossing.depth_cm,
                threshold_f: crossing.threshold_f,
                direction: crossing.direction,
                calibration,
                median_days: errors.median,
                p80_days: errors.p80,
                bias_days: round(errors.bias),
                spread_median_days: between_years.median,
                spread_p80_days: between_years.p80,
                samples: errors.samples,
            })
        })
        .collect()
}

fn round(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}
