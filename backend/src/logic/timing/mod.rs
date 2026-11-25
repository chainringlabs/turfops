//! Seeding and pre-emergent timing windows.
//!
//! Every window is a pair of boundaries (plus an optional ideal stretch) defined on the
//! 5-day mean soil temperature at 5 cm, the station's freeze climatology, or running
//! GDD. Each boundary gets a *typical* date — found per historical year, then summarized
//! as median/spread — and a standing for the current season: observed in station data,
//! tentative, forecast, or still on its typical date. Pure functions; the handler in
//! `api/timing.rs` supplies the data.

pub mod climatology;
pub mod companions;
pub mod context;
pub mod evaluate;
pub mod log;
pub mod plan;
pub mod recommendations;
pub mod series;
pub mod text;
pub mod windows;

#[cfg(test)]
mod tests;

use crate::logic::soil_estimate::anchor::{forward, Days};
use crate::logic::soil_estimate::model::{DailyAir, Model};
use crate::logic::soil_estimate::scores::Calibration;
use crate::logic::soil_temp_prediction::{anchored_outlook, continuous_air, fit_model};
use crate::models::climate::ClimateDay;
use crate::models::timing::{FreezeDates, SoilNow, TimingWindow, WindowState};
use crate::models::{Application, DailyForecast, GrassType};
use chrono::{Datelike, Duration, NaiveDate};
use evaluate::{Season, WindowViews};
use series::SoilPoint;

/// Years of station history requested for the typical dates.
pub const HISTORY_YEARS: i32 = 15;
/// Soil depth the windows are defined on.
pub const SOIL_DEPTH_CM: u32 = 5;
/// Days of paired air/soil data used to fit the soil forecast.
const FORECAST_TRAINING_DAYS: i64 = 30;
/// Station soil data older than this cannot tell us what has happened this season.
const STALE_AFTER_DAYS: i64 = 5;
/// Measured soil days among the training days for the fitted soil forecast.
const MIN_MEASURED_TRAINING_DAYS: usize = 14;
/// Recorded air behind the estimate's forward series: its longest trailing mean is 30 days.
const ESTIMATE_AIR_DAYS: i64 = 60;

/// The stored 5 cm soil estimate: its model, and the calibration it was fitted under.
pub struct SoilEstimate {
    pub model: Model,
    pub calibration: Calibration,
}

pub struct Inputs<'a> {
    pub today: NaiveDate,
    /// Ascending daily climate record, `HISTORY_YEARS` back through the latest day.
    pub days: &'a [ClimateDay],
    pub forecast: &'a [DailyForecast],
    pub grass: GrassType,
    /// Applications from at least the previous July onward.
    pub history: &'a [Application],
    /// The stored 5 cm soil estimate, when there is one.
    pub estimate: Option<&'a SoilEstimate>,
}

pub struct Assessment {
    pub windows: Vec<TimingWindow>,
    pub soil: SoilNow,
    pub freeze: FreezeDates,
    pub history_years: Vec<i32>,
    pub smoothed: Vec<SoilPoint>,
    pub data_fresh: bool,
    /// Windows left out because the lawn's soil basis cannot date them (TIME-ENG-050).
    pub withheld: Vec<String>,
}

fn observed_points(days: &[ClimateDay]) -> Vec<SoilPoint> {
    days.iter()
        .filter_map(|d| {
            Some(SoilPoint {
                date: d.date,
                temp_f: d.soil_temp_5_f?,
                is_forecast: false,
                estimated: d.soil_estimated,
            })
        })
        .collect()
}

// @spec TIME-ENG-007, TIME-ENG-008
/// 5 cm soil for each day after the last observation through the end of the forecast.
/// While the last soil day is measured and enough of the month before it is, the lagged
/// air→soil regression fit on those measured days, anchored to the last soil value;
/// otherwise the soil estimate's forward series. Empty with no soil, no forecast, or
/// neither a fit nor an estimate.
fn forecast_points(
    days: &[ClimateDay],
    forecast: &[DailyForecast],
    estimate: Option<&SoilEstimate>,
) -> Vec<SoilPoint> {
    let Some(last) = days.iter().rev().find(|d| d.soil_temp_5_f.is_some()) else {
        return Vec::new();
    };
    let (last_date, last_temp) = (last.date, last.soil_temp_5_f.unwrap_or_default());
    let Some(last_forecast_day) = forecast.last().map(|f| f.date) else {
        return Vec::new();
    };
    let forecast_air: Vec<(NaiveDate, f64)> = forecast
        .iter()
        .map(|f| (f.date, (f.high_temp_f + f.low_temp_f) / 2.0))
        .collect();
    let recorded_air = |from: NaiveDate| -> Vec<(NaiveDate, f64)> {
        days.iter()
            .filter(|d| d.date >= from)
            .filter_map(|d| Some((d.date, d.air_avg_f?)))
            .collect()
    };
    let point = |date: NaiveDate, temp_f: f64| SoilPoint {
        date,
        temp_f,
        is_forecast: true,
        estimated: false,
    };

    let training_start = last_date - Duration::days(FORECAST_TRAINING_DAYS);
    let pairs: Vec<(NaiveDate, f64, f64)> = days
        .iter()
        .filter(|d| d.date >= training_start && !d.soil_estimated)
        .filter_map(|d| Some((d.date, d.air_avg_f?, d.soil_temp_5_f?)))
        .collect();
    let measured = !last.soil_estimated && pairs.len() >= MIN_MEASURED_TRAINING_DAYS;
    if measured {
        let Some(model) = fit_model(&pairs) else {
            return Vec::new();
        };
        let air = continuous_air(&recorded_air(training_start), &forecast_air);
        // The timing outlook ignores per-prediction confidence, so its "today" does not
        // matter.
        return anchored_outlook(&model, &air, (last_date, last_temp), last_date)
            .into_iter()
            .map(|p| point(p.date, p.predicted_soil_temp_f))
            .collect();
    }
    let Some(estimate) = estimate else {
        return Vec::new();
    };
    let air: DailyAir = continuous_air(
        &recorded_air(last_date - Duration::days(ESTIMATE_AIR_DAYS)),
        &forecast_air,
    )
    .into_iter()
    .collect();
    let measured_soil: Days = days
        .iter()
        .filter(|d| !d.soil_estimated)
        .filter_map(|d| Some((d.date, d.soil_temp_5_f?)))
        .collect();
    forward(&estimate.model, &air, &measured_soil, last_date)
        .into_iter()
        .filter(|(date, _)| *date <= last_forecast_day)
        .map(|(date, temp_f)| point(date, temp_f))
        .collect()
}

// @spec TIME-LOG-003
fn evaluate_window(
    spec: &windows::WindowSpec,
    season: &Season,
    history: &[Application],
) -> TimingWindow {
    let season_year = windows::season_year(spec.id, season.today);
    let views = WindowViews::build(spec, season, season_year);
    let logged = log::check(spec.id, season_year, history);
    let state = if logged.done_on.is_some() {
        WindowState::Done
    } else if logged.conflict.is_some() {
        WindowState::Blocked
    } else {
        views.weather_state(season.today)
    };

    TimingWindow {
        id: spec.id,
        name: spec.name.to_string(),
        priority: spec.priority,
        season_year,
        state,
        headline: text::headline(state, &views, logged.done_on),
        detail: text::detail(state, &views, season.today),
        guidance: text::guidance(spec.id, state),
        opens: views.opens,
        ideal_from: views.ideal_from,
        ideal_until: views.ideal_until,
        closes: views.closes,
        done_on: logged.done_on,
        conflict: logged.conflict,
    }
}

/// The smoothed soil series and climatology that every evaluation shares.
pub struct SeasonData {
    pub smoothed: Vec<SoilPoint>,
    pub history_years: Vec<i32>,
    pub spring_freezes: Vec<(i32, NaiveDate)>,
    pub fall_freezes: Vec<(i32, NaiveDate)>,
    last_observed: Option<NaiveDate>,
    calibration: Option<Calibration>,
}

impl SeasonData {
    /// The series and climatology with no stored soil estimate.
    #[cfg(test)]
    pub fn build(days: &[ClimateDay], forecast: &[DailyForecast], current_year: i32) -> Self {
        Self::build_with(days, forecast, current_year, None)
    }

    // @spec TIME-ENG-006
    pub fn build_with(
        days: &[ClimateDay],
        forecast: &[DailyForecast],
        current_year: i32,
        estimate: Option<&SoilEstimate>,
    ) -> Self {
        let mut points = observed_points(days);
        let last_observed = points.last().map(|p| p.date);
        points.extend(forecast_points(days, forecast, estimate));
        let history_years = climatology::history_years(days, current_year);
        Self {
            smoothed: series::smooth(&points),
            spring_freezes: climatology::spring_freeze_history(days, &history_years),
            fall_freezes: climatology::fall_freeze_history(days, &history_years),
            history_years,
            last_observed,
            calibration: estimate.map(|e| e.calibration),
        }
    }

    // @spec TIME-ENG-021
    pub fn season<'a>(&'a self, days: &'a [ClimateDay], today: NaiveDate) -> Season<'a> {
        Season {
            today,
            days,
            smoothed: &self.smoothed,
            history_years: &self.history_years,
            fall_freezes: &self.fall_freezes,
            data_fresh: self
                .last_observed
                .is_some_and(|d| (today - d).num_days() <= STALE_AFTER_DAYS),
            last_observed: self.last_observed,
            calibration: self.calibration,
        }
    }
}

/// Every soil crossing among a boundary's triggers.
fn soil_crossings(boundary: &windows::Boundary) -> Vec<&series::Crossing> {
    match boundary {
        windows::Boundary::Soil(crossing) => vec![crossing],
        windows::Boundary::Earliest(options) => options.iter().flat_map(soil_crossings).collect(),
        windows::Boundary::BeforeFirstFreeze(_) | windows::Boundary::Gdd(_) => Vec::new(),
    }
}

// @spec TIME-ENG-050
/// While the lawn's soil rests on the estimate anywhere in the record, a window with a
/// soil trigger the estimate may not date and no history year measured cannot be
/// offered.
fn undatable(spec: &windows::WindowSpec, season: &Season, on_estimate: bool) -> bool {
    on_estimate
        && [
            Some(&spec.opens),
            spec.ideal_from.as_ref(),
            spec.ideal_until.as_ref(),
            Some(&spec.closes),
        ]
        .into_iter()
        .flatten()
        .flat_map(soil_crossings)
        .any(|c| season.estimate_error_days(c).is_none() && !season.measured_history(c))
}

pub fn assess(inputs: &Inputs) -> Assessment {
    let today = inputs.today;
    let data = SeasonData::build_with(inputs.days, inputs.forecast, today.year(), inputs.estimate);
    let season = data.season(inputs.days, today);
    let data_fresh = season.data_fresh;
    let on_estimate = inputs.days.iter().any(|d| d.soil_estimated);
    let specs = windows::specs_for(inputs.grass);
    let (offered, withheld): (Vec<_>, Vec<_>) = specs
        .iter()
        .partition(|spec| !undatable(spec, &season, on_estimate));
    let windows = offered
        .into_iter()
        .map(|spec| evaluate_window(spec, &season, inputs.history))
        .collect();

    let latest = data.smoothed.iter().rev().find(|p| !p.is_forecast);
    Assessment {
        windows,
        soil: SoilNow {
            avg_5day_f: latest.map(|p| (p.temp_f * 10.0).round() / 10.0),
            as_of: latest.map(|p| p.date),
            depth_cm: SOIL_DEPTH_CM,
        },
        freeze: FreezeDates {
            last_spring: climatology::date_stat(&data.spring_freezes, today.year()),
            first_fall: climatology::date_stat(&data.fall_freezes, today.year()),
            last_spring_this_year: climatology::last_spring_freeze(inputs.days, today.year())
                .filter(|_| today.month() >= 6),
            first_fall_this_year: climatology::first_fall_freeze(inputs.days, today.year()),
        },
        history_years: data.history_years,
        smoothed: data.smoothed,
        data_fresh,
        withheld: withheld.iter().map(|spec| spec.name.to_string()).collect(),
    }
}
