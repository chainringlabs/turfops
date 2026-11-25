//! Soil that rests on the estimate: estimated runs, what the estimate may date, the
//! windows it cannot, and the soil outlook it drives.

use super::*;
use crate::logic::soil_estimate::model::Model;
use crate::logic::soil_estimate::scores::Calibration;
use crate::logic::timing::evaluate::{resolve, typical};
use crate::logic::timing::series::{detect, Crossing, Direction, SeasonDay, SoilPoint};
use crate::logic::timing::windows::{specs_for, Boundary, WindowSpec};
use crate::models::timing::WindowPriority;

/// Soil = yesterday's-and-today's air mean + `offset`.
fn estimate(calibration: Calibration, offset: f64) -> SoilEstimate {
    SoilEstimate {
        model: Model {
            coefficients: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, offset],
        },
        calibration,
    }
}

/// `days` with the soil of every day `pick` chooses resting on the estimate.
fn estimated(mut days: Vec<ClimateDay>, pick: impl Fn(NaiveDate) -> bool) -> Vec<ClimateDay> {
    for day in &mut days {
        day.soil_estimated = pick(day.date);
    }
    days
}

fn fall_pre_em() -> WindowSpec {
    specs_for(GrassType::TallFescue)
        .into_iter()
        .find(|s| s.id == WindowId::FallPreEmergent)
        .unwrap()
}

/// 5 cm soil cooling through 61 °F, a threshold the estimate was never scored on.
const UNSCORED: Crossing = Crossing {
    threshold_f: 61.0,
    direction: Direction::Falling,
    scan_from: SeasonDay::new(0, 8, 1),
    scan_to: SeasonDay::new(0, 12, 31),
};

fn today() -> NaiveDate {
    date(2026, 9, 20)
}

// @spec TIME-ENG-047
#[test]
fn a_run_is_estimated_when_one_of_its_first_five_days_is() {
    let point = |day: u32, estimated: bool| SoilPoint {
        date: date(2026, 9, day),
        temp_f: 60.0,
        is_forecast: false,
        estimated,
    };
    let crossing = Crossing {
        threshold_f: 70.0,
        ..UNSCORED
    };
    let third = vec![
        point(1, false),
        point(2, false),
        point(3, true),
        point(4, false),
        point(5, false),
    ];
    assert!(detect(&third, 2026, &crossing).unwrap().estimated);
    let sixth: Vec<SoilPoint> = (1..=6).map(|day| point(day, day == 6)).collect();
    let run = detect(&sixth, 2026, &crossing).unwrap();
    assert!(
        run.confirmed() && !run.estimated,
        "dated on its first five days"
    );
}

// @spec TIME-ENG-048, TIME-ENG-051
#[test]
fn a_datable_estimated_run_is_dated_as_a_measured_one_and_marked() {
    let measured = station(date(2026, 9, 19), 0.0);
    let all_estimated = estimated(measured.clone(), |_| true);
    let fit = estimate(Calibration::NearestFive, 5.0);
    let opens = fall_pre_em().opens;

    let plain = SeasonData::build(&measured, &[], 2026);
    let plain = resolve(&opens, &plain.season(&measured, today()), 2026);
    let data = SeasonData::build_with(&all_estimated, &[], 2026, Some(&fit));
    let season = data.season(&all_estimated, today());
    let marked = resolve(&opens, &season, 2026);

    assert_eq!((marked.date, marked.source), (plain.date, plain.source));
    assert!(!plain.estimated && plain.typical_error_days.is_none());
    assert!(marked.estimated);
    // 5 cm soil cooling to 70 °F, nearest-five calibration: 2 days.
    assert_eq!(marked.typical_error_days, Some(2.0));
    let stat = typical(&opens, &season, 2026).unwrap();
    assert_eq!(stat.estimated_years, stat.sample_count);
}

// @spec TIME-ENG-051
#[test]
fn typical_dates_count_the_years_dated_from_the_estimate() {
    let days = estimated(station(date(2026, 9, 19), 0.0), |d| {
        (2021..=2022).contains(&d.year())
    });
    let fit = estimate(Calibration::StationAlone, 5.0);
    let data = SeasonData::build_with(&days, &[], 2026, Some(&fit));
    let stat = typical(&fall_pre_em().opens, &data.season(&days, today()), 2026).unwrap();
    assert_eq!((stat.sample_count, stat.estimated_years), (6, 2));
}

// @spec TIME-ENG-049
#[test]
fn an_estimated_run_the_estimate_may_not_date_is_no_run_at_all() {
    let measured = station(date(2026, 10, 9), 0.0);
    let all_estimated = estimated(measured.clone(), |_| true);
    let boundary = Boundary::Soil(UNSCORED);

    let data = SeasonData::build(&measured, &[], 2026);
    let season = data.season(&measured, date(2026, 10, 10));
    assert_eq!(
        resolve(&boundary, &season, 2026).source,
        DateSource::Observed
    );

    let fit = estimate(Calibration::NearestFive, 5.0);
    for fit in [Some(&fit), None] {
        let data = SeasonData::build_with(&all_estimated, &[], 2026, fit);
        let season = data.season(&all_estimated, date(2026, 10, 10));
        let resolved = resolve(&boundary, &season, 2026);
        assert_eq!(resolved.source, DateSource::Typical);
        assert!(!resolved.estimated);
        assert!(
            typical(&boundary, &season, 2026).is_none(),
            "no history year dates it either"
        );
    }
}

// @spec TIME-ENG-050
#[test]
fn a_window_the_estimate_cannot_date_and_no_measured_year_dates_is_withheld() {
    let spec = WindowSpec {
        id: WindowId::FallPreEmergent,
        name: "Test window",
        priority: WindowPriority::Primary,
        opens: Boundary::Soil(UNSCORED),
        ideal_from: None,
        ideal_until: None,
        closes: Boundary::Gdd(200.0),
    };
    let fit = estimate(Calibration::NearestFive, 5.0);
    let measured = station(date(2026, 9, 19), 0.0);
    let all_estimated = estimated(measured.clone(), |_| true);

    let data = SeasonData::build_with(&all_estimated, &[], 2026, Some(&fit));
    assert!(undatable(
        &spec,
        &data.season(&all_estimated, today()),
        true
    ));
    // Measured history dates it, so it stays.
    let this_year = estimated(measured.clone(), |d| d.year() == 2026);
    let data = SeasonData::build_with(&this_year, &[], 2026, Some(&fit));
    assert!(!undatable(&spec, &data.season(&this_year, today()), true));
    // Soil that never rests on the estimate never withholds a window.
    let data = SeasonData::build(&measured, &[], 2026);
    assert!(!undatable(&spec, &data.season(&measured, today()), false));

    // Every real window is datable, so none is withheld.
    let assessment = assess(&Inputs {
        today: today(),
        days: &all_estimated,
        forecast: &[],
        grass: GrassType::TallFescue,
        history: &[],
        estimate: Some(&fit),
    });
    assert!(assessment.withheld.is_empty());
    assert_eq!(assessment.windows.len(), 5);
}

// @spec TIME-ENG-007, TIME-ENG-008
#[test]
fn without_enough_measured_soil_the_outlook_is_the_estimates() {
    let last = date(2026, 9, 19);
    let forecast: Vec<DailyForecast> = [55.0, 58.0, 61.0]
        .iter()
        .enumerate()
        .map(|(i, &avg)| forecast_day(date(2026, 9, 20) + Duration::days(i as i64), avg))
        .collect();
    let fit = estimate(Calibration::NearestFive, 9.0);

    // Soil from the estimate: no fit, so nothing without the stored estimate.
    let all_estimated = estimated(station(last, 0.0), |_| true);
    assert!(forecast_points(&all_estimated, &forecast, None).is_empty());
    let outlook = forecast_points(&all_estimated, &forecast, Some(&fit));
    let got: Vec<(NaiveDate, f64)> = outlook.iter().map(|p| (p.date, p.temp_f)).collect();
    assert_eq!(got.len(), 3, "{got:?}");
    for (point, air) in outlook.iter().zip([55.0, 58.0, 61.0]) {
        assert!(point.is_forecast && !point.estimated);
        assert!((point.temp_f - (air + 9.0)).abs() < 0.11, "{point:?}");
    }

    // The last day measured but fewer than 14 of the last 30: still the estimate's path.
    let mostly_estimated = estimated(station(last, 0.0), |d| d > date(2026, 8, 31) && d < last);
    assert!(forecast_points(&mostly_estimated, &forecast, None).is_empty());
    assert_eq!(
        forecast_points(&mostly_estimated, &forecast, Some(&fit)).len(),
        3
    );
}
