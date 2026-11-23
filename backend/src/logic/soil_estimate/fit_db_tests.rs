//! Fitting and writing the estimate against PostgreSQL, on weather made from a known
//! model so the fit can be checked against it.

use super::calibration::Decision;
use super::estimator::SoilEstimator;
use super::estimator_db_tests::neighbours;
use super::model::{DailyAir, Model};
use super::scores::Calibration;
use crate::db::soil_estimate_queries;
use crate::db::weather_queries::{days, rows};
use crate::logic::weather_record::lab::*;
use crate::models::weather_record::*;
use chrono::{Datelike, Duration, NaiveDate};
use chrono_tz::America::New_York;
use sqlx::PgPool;

/// The relation the made-up soil follows at 5 cm; 10 cm runs 2 °F under it.
const TRUE: Model = Model {
    coefficients: [0.2, 0.2, 0.2, 0.1, 0.1, -3.0, -2.0, 12.0],
};
const DAYS: i64 = 1150;

fn first_day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2011, 12, 1).unwrap()
}

/// A place's daily air: a season with noise that differs by `seed`.
fn air(seed: u64) -> DailyAir {
    (0..DAYS)
        .map(|n| {
            let mixed = (n as u64 + 1)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(seed.wrapping_mul(1442695040888963407));
            let noise = ((mixed >> 33) % 1600) as f64 / 100.0 - 8.0;
            let season = 20.0 * (std::f64::consts::TAU * n as f64 / 365.25).sin();
            (first_day() + Duration::days(n), 55.0 + season + noise)
        })
        .collect()
}

fn grid_rows(air: &DailyAir) -> Vec<DailyRow> {
    air.iter()
        .map(|(day, mean)| DailyRow {
            day: *day,
            air_max_f: Some(mean + 10.0),
            air_min_f: Some(mean - 10.0),
            ..Default::default()
        })
        .collect()
}

/// What the true model makes of `air` at 5 cm, for each day it can.
fn soil(air: &DailyAir) -> Vec<(NaiveDate, f64)> {
    TRUE.series(air)
}

async fn write_daily(lab: &Lab, key: &str, rows: &[DailyRow]) {
    let source = lab.source(key).await;
    let mut conn = lab.pool.acquire().await.unwrap();
    rows::write_daily(&mut conn, &source, rows).await.unwrap();
    sqlx::query(
        "UPDATE weather_fetch_state SET held_from = wanted_from, held_through = $2 \
         WHERE source_id = $1",
    )
    .bind(source.id)
    .bind(d(19))
    .execute(&lab.pool)
    .await
    .unwrap();
}

/// The lab with six neighbours, the five nearest holding soil that follows the true
/// model on the air at their own points, and the lawn's grid holding its own air.
async fn calibrated(pool: PgPool) -> (Lab, SoilEstimator) {
    let lab = Lab::new(pool).await;
    neighbours(&lab).await;
    let estimator = SoilEstimator::new(lab.pool.clone());
    estimator.prepare(d(20)).await.unwrap();
    let catalog = crate::db::weather_queries::catalog::stations(&lab.pool)
        .await
        .unwrap();
    for (seed, id) in ["b", "c", "d", "e", "f"].into_iter().enumerate() {
        let here = air(seed as u64 + 1);
        let station = &catalog.iter().find(|(_, s)| s.station_id == id).unwrap().1;
        let grid = format!(
            "calibration:grid:prism:{:.4},{:.4}",
            station.latitude, station.longitude
        );
        write_daily(&lab, &grid, &grid_rows(&here)).await;
        let soil_rows: Vec<DailyRow> = soil(&here)
            .into_iter()
            .map(|(day, value)| DailyRow {
                day,
                soil_temp_5_f: Some(value),
                soil_temp_10_f: Some(value - 2.0),
                ..Default::default()
            })
            .collect();
        write_daily(&lab, &format!("calibration:Uscrn:{id}"), &soil_rows).await;
    }
    write_daily(&lab, PRISM, &grid_rows(&air(99))).await;
    (lab, estimator)
}

async fn estimate_rows(lab: &Lab) -> Vec<DailyRow> {
    let id = lab.source(ESTIMATE).await.id;
    rows::daily(&lab.pool, id, first_day(), d(20))
        .await
        .unwrap()
}

// @spec WX-EST-CAL-001, WX-EST-CAL-003, WX-EST-CAL-004, WX-EST-CAL-006, WX-EST-MODEL-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_five_nearest_are_fitted_and_the_series_is_written_from_the_lawns_air(pool: PgPool) {
    let (lab, estimator) = calibrated(pool).await;
    let now = noon(20).at.now;
    let decisions = estimator.run(d(20), now).await.unwrap();
    assert!(decisions
        .iter()
        .all(|(_, d)| matches!(d, Decision::Fit(Calibration::NearestFive, _))));

    let fits = soil_estimate_queries::all(&lab.pool).await.unwrap();
    assert_eq!(
        fits.iter().map(|f| f.depth_cm).collect::<Vec<_>>(),
        vec![5, 10]
    );
    let five = &fits[0];
    assert_eq!(five.calibration, Calibration::NearestFive);
    assert_eq!(five.fitted_at, now);
    let names: Vec<_> = five.stations.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names[0], "Station b, PA");
    assert_eq!(names.len(), 5);
    assert!((five.stations[0].distance_miles - 30.0).abs() < 0.05);
    assert!(
        five.rmse_f < 0.06,
        "only the 0.1 °F rounding is left: {}",
        five.rmse_f
    );
    // Each station gives a row for every day from 2012 that the model can reach.
    let per_station = soil(&air(1))
        .iter()
        .filter(|(day, _)| day.year_ce().1 >= 2012)
        .count();
    assert_eq!(five.training_days as usize, 5 * per_station);
    for (found, truth) in five.model.coefficients.iter().zip(TRUE.coefficients) {
        assert!((found - truth).abs() < 0.2, "{found} against {truth}");
    }

    // The series is the lawn's own air through the model.
    let written = estimate_rows(&lab).await;
    let expected = soil(&air(99));
    assert_eq!(written.len(), expected.len());
    for (row, (day, value)) in written.iter().zip(&expected) {
        assert_eq!(row.day, *day);
        assert!((row.soil_temp_5_f.unwrap() - value).abs() <= 0.11, "{day}");
        assert!((row.soil_temp_10_f.unwrap() - (value - 2.0)).abs() <= 0.11);
    }

    // And the record answers from it.
    let asked = noon(20);
    let from = expected[0].0;
    let series = lab
        .record
        .fetch_daily_soil_temp_averages(from, from + Duration::days(9), asked)
        .await
        .unwrap();
    assert_eq!(series.rows.len(), 10);
    assert_eq!(series.basis[0].basis.source, "Soil temperature estimate");
}

// @spec WX-EST-CAL-005, WX-EST-FAIL-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_model_is_refitted_monthly_and_a_failed_refit_keeps_the_fit_before_it(pool: PgPool) {
    let (lab, estimator) = calibrated(pool).await;
    let now = noon(20).at.now;
    let fitted_at = |lab: &Lab| {
        let pool = lab.pool.clone();
        async move { soil_estimate_queries::all(&pool).await.unwrap()[0].fitted_at }
    };
    estimator.run(d(20), now).await.unwrap();
    estimator
        .run(d(20), now + Duration::days(29))
        .await
        .unwrap();
    assert_eq!(fitted_at(&lab).await, now, "not yet a month old");
    let month_on = now + Duration::days(30);
    estimator.run(d(20), month_on).await.unwrap();
    assert_eq!(fitted_at(&lab).await, month_on);

    // The grids at the stations' points lose their rows: nothing can be paired.
    sqlx::query(
        "DELETE FROM weather_daily WHERE source_id IN \
         (SELECT id FROM weather_source WHERE key LIKE 'calibration:grid:%')",
    )
    .execute(&lab.pool)
    .await
    .unwrap();
    estimator
        .run(d(20), month_on + Duration::days(31))
        .await
        .unwrap();
    assert_eq!(fitted_at(&lab).await, month_on, "the earlier fit stands");
    assert!(!estimate_rows(&lab).await.is_empty());
}

// @spec WX-EST-FAIL-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn while_calibration_data_is_awaited_there_is_no_fit_and_no_estimate(pool: PgPool) {
    let lab = Lab::new(pool).await;
    neighbours(&lab).await;
    let estimator = SoilEstimator::new(lab.pool.clone());
    let decisions = estimator.run(d(20), noon(20).at.now).await.unwrap();
    assert!(decisions
        .iter()
        .all(|(_, d)| matches!(d, Decision::Wait(_))));
    assert!(soil_estimate_queries::all(&lab.pool)
        .await
        .unwrap()
        .is_empty());
    assert!(estimate_rows(&lab).await.is_empty());
}

// @spec WX-EST-CAL-002, WX-EST-MODEL-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_basis_station_is_fitted_on_alone_and_the_series_runs_on_the_air_at_its_point(
    pool: PgPool,
) {
    let lab = Lab::new(pool).await;
    let estimator = SoilEstimator::new(lab.pool.clone());
    let there = air(7);
    // Twelve hours a day of the station's soil, following the true model on its own air.
    let hours: Vec<HourlyRow> = soil(&there)
        .into_iter()
        .flat_map(|(day, value)| {
            let start = days::start_of_day(New_York, day);
            (0..12i64).map(move |hour| {
                let mut row = HourlyRow {
                    observed_at: start + Duration::hours(hour),
                    ..Default::default()
                };
                row.soil_temp_f = [Some(value), Some(value - 2.0), None, None, None];
                row
            })
        })
        .collect();
    lab.write_hours(STATION, &hours).await;
    // The lawn's own grid is a different, hotter place.
    let lawn: DailyAir = there.keys().map(|day| (*day, 90.0)).collect();
    write_daily(&lab, PRISM, &grid_rows(&lawn)).await;

    // The first pass asks for the grid at the station's point; then it arrives.
    estimator.run(d(20), noon(20).at.now).await.unwrap();
    assert!(soil_estimate_queries::all(&lab.pool)
        .await
        .unwrap()
        .is_empty());
    let grid = crate::db::weather_queries::sources::all(&lab.pool)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.key.starts_with("calibration:grid"))
        .unwrap();
    write_daily(&lab, &grid.key, &grid_rows(&there)).await;

    let decisions = estimator.run(d(20), noon(20).at.now).await.unwrap();
    assert!(decisions
        .iter()
        .all(|(_, d)| matches!(d, Decision::Fit(Calibration::StationAlone, _))));
    let fits = soil_estimate_queries::all(&lab.pool).await.unwrap();
    assert_eq!(fits[1].stations[0].name, "Avondale 2 N, PA");
    assert!(fits[1].rmse_f < 0.06);

    let written = estimate_rows(&lab).await;
    let from_2012: Vec<_> = soil(&there)
        .into_iter()
        .filter(|(day, _)| day.year_ce().1 >= 2012)
        .collect();
    let last = written.last().unwrap();
    let truth = from_2012.last().unwrap();
    assert_eq!(last.day, truth.0);
    assert!((last.soil_temp_10_f.unwrap() - (truth.1 - 2.0)).abs() <= 0.11);
    assert!(
        last.soil_temp_5_f.unwrap() < 80.0,
        "not the lawn grid's 90 °F air"
    );
}
