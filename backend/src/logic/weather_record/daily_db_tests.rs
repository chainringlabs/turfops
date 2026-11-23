//! The record's daily contracts against PostgreSQL.

use super::lab::*;
use super::record::WeatherRecord;
use crate::error::{Outage, TurfOpsError};
use crate::models::weather_basis::BasisKind;
use sqlx::PgPool;

pub(super) fn outage(err: TurfOpsError) -> Outage {
    match err {
        TurfOpsError::Unavailable { outage, .. } => outage,
        other => panic!("not an outage: {other:?}"),
    }
}

// @spec WX-REC-FAIL-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_lawn_with_no_location_gets_the_location_outage_from_every_contract(pool: PgPool) {
    let record = WeatherRecord::new(pool);
    let asked = noon(20);
    let errors = [
        record
            .fetch_daily_soil_temp_averages(d(1), d(20), asked)
            .await
            .unwrap_err(),
        record
            .fetch_daily_paired_averages(d(1), d(20), asked)
            .await
            .unwrap_err(),
        record
            .fetch_daily_gdd(d(1), d(20), asked)
            .await
            .unwrap_err(),
        record.fetch_gdd_ytd(asked).await.unwrap_err(),
        record.fetch_climate_record(d(1), asked).await.unwrap_err(),
    ];
    for err in errors {
        assert!(err
            .owner_message()
            .contains("doesn't know where your lawn is"));
        assert_eq!(outage(err), Outage::LocationNotSet);
    }
}

/// The station covers the 10th to the 17th but for six hours on the 15th; the estimate
/// covers the 10th to the 19th.
async fn soil_lab(pool: PgPool) -> Lab {
    let lab = Lab::new(pool).await;
    for day in 10..=17 {
        let hours = if day == 15 { 6 } else { 24 };
        lab.station_hours(day, hours, 70.0).await;
    }
    for day in 10..=19 {
        lab.estimate(day, 60.0).await;
    }
    lab
}

// @spec WX-REC-API-005, WX-REC-RES-001, WX-REC-RES-010, WX-REC-BASIS-001, WX-REC-BASIS-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn soil_days_come_from_the_station_and_its_holes_from_the_estimate(pool: PgPool) {
    let lab = soil_lab(pool).await;
    let series = lab
        .record
        .fetch_daily_soil_temp_averages(d(1), d(31), noon(20))
        .await
        .unwrap();
    let rows: Vec<_> = series.rows.iter().map(|r| (r.date, r.avg_temp_f)).collect();
    let expected: Vec<_> = (10..=17)
        // The 15th is the estimate, anchored to the station's 70 °F on either side.
        .map(|day| (d(day), 70.0))
        .collect();
    assert_eq!(rows, expected, "the 18th and 19th wait for the station");
    let basis: Vec<_> = series
        .basis
        .iter()
        .map(|s| (s.from, s.to, s.basis.kind, s.basis.source.as_str()))
        .collect();
    assert_eq!(
        basis,
        vec![
            (d(10), d(14), BasisKind::Station, "Avondale 2 N, PA (USCRN)"),
            (
                d(15),
                d(15),
                BasisKind::Estimate,
                "Soil temperature estimate"
            ),
            (d(16), d(17), BasisKind::Station, "Avondale 2 N, PA (USCRN)"),
        ]
    );
    assert!((series.basis[0].basis.distance_miles.unwrap() - 20.0).abs() < 0.05);
}

// @spec WX-REC-RES-010, WX-REC-API-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn once_the_station_is_past_its_age_limit_the_estimate_takes_its_missing_days(pool: PgPool) {
    let lab = soil_lab(pool).await;
    // Four days after the station's last hour; today is the 22nd.
    let series = lab
        .record
        .fetch_daily_soil_temp_averages(d(1), d(31), noon(22))
        .await
        .unwrap();
    let last: Vec<_> = series
        .rows
        .iter()
        .rev()
        .take(3)
        .map(|r| (r.date, r.avg_temp_f))
        .collect();
    assert_eq!(last, vec![(d(19), 69.3), (d(18), 69.7), (d(17), 70.0)]);
}

// @spec WX-REC-API-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_day_in_progress_is_not_in_a_daily_series(pool: PgPool) {
    let lab = Lab::new(pool).await;
    for day in 18..=20 {
        lab.estimate(day, 60.0).await;
    }
    let series = lab
        .record
        .fetch_daily_soil_temp_averages(d(1), d(31), noon(20))
        .await
        .unwrap();
    let days: Vec<_> = series.rows.iter().map(|r| r.date).collect();
    assert_eq!(days, vec![d(18), d(19)]);
}

/// PRISM has the 10th to the 18th; the NRCC grid has the 19th and the 20th.
pub(super) async fn air_lab(pool: PgPool) -> Lab {
    let lab = Lab::new(pool).await;
    for day in 10..=18 {
        lab.grid(PRISM, day, 80.0, 60.0).await;
    }
    lab.grid(NRCC, 18, 99.0, 99.0).await;
    lab.grid(NRCC, 19, 90.0, 70.0).await;
    lab.grid(NRCC, 20, 60.0, 30.0).await;
    lab
}

// @spec WX-REC-API-008, WX-REC-RES-007, WX-REC-FAIL-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn daily_air_is_the_first_grid_that_has_the_day_and_gdd_never_goes_below_zero(pool: PgPool) {
    let lab = air_lab(pool).await;
    let series = lab
        .record
        .fetch_daily_gdd(d(17), d(20), noon(20))
        .await
        .unwrap();
    assert_eq!(
        series.rows,
        vec![
            (d(17), 80.0, 60.0, 20.0),
            (d(18), 80.0, 60.0, 20.0),
            (d(19), 90.0, 70.0, 30.0),
            (d(20), 60.0, 30.0, 0.0),
        ]
    );
    let sources: Vec<_> = series
        .basis
        .iter()
        .map(|s| s.basis.source.as_str())
        .collect();
    assert_eq!(sources, vec!["PRISM daily grid", "NRCC daily grid"]);
}

// @spec WX-REC-API-009, WX-REC-BASIS-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn gdd_to_date_carries_its_newest_days_basis_and_says_when_it_is_mixed(pool: PgPool) {
    let lab = air_lab(pool).await;
    let figure = lab.record.fetch_gdd_ytd(noon(20)).await.unwrap().unwrap();
    assert_eq!(figure.value, 9.0 * 20.0 + 30.0);
    assert_eq!(figure.basis.source, "NRCC daily grid");
    assert!(figure.mixed);
    let one_basis = lab.record.fetch_gdd_ytd(noon(18)).await.unwrap().unwrap();
    assert_eq!((one_basis.value, one_basis.mixed), (180.0, false));
}

// @spec WX-REC-API-009
#[sqlx::test(migrations = "src/db/migrations")]
async fn gdd_to_date_is_nothing_when_no_day_has_a_value(pool: PgPool) {
    let lab = Lab::new(pool).await;
    assert_eq!(lab.record.fetch_gdd_ytd(noon(20)).await.unwrap(), None);
}

// @spec WX-REC-API-006, WX-REC-API-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn paired_days_are_the_days_with_both_air_and_soil(pool: PgPool) {
    let lab = air_lab(pool).await;
    lab.estimate(9, 55.0).await;
    lab.estimate(12, 61.0).await;
    lab.estimate(13, 62.0).await;
    let series = lab
        .record
        .fetch_daily_paired_averages(d(1), d(31), noon(20))
        .await
        .unwrap();
    assert_eq!(series.rows, vec![(d(12), 70.0, 61.0), (d(13), 70.0, 62.0)]);
    let spans: Vec<_> = series
        .basis
        .iter()
        .map(|s| (s.input, s.from, s.to))
        .collect();
    assert_eq!(
        spans,
        vec![
            (crate::models::weather_record::Input::DailyAir, d(12), d(13)),
            (
                crate::models::weather_record::Input::SoilTemp10,
                d(12),
                d(13)
            ),
        ]
    );
}

// @spec WX-REC-API-011
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_climate_record_keeps_a_day_that_has_only_one_side(pool: PgPool) {
    let lab = air_lab(pool).await;
    lab.estimate(9, 55.0).await;
    lab.estimate(10, 56.0).await;
    let series = lab
        .record
        .fetch_climate_record(d(9), noon(12))
        .await
        .unwrap();
    let rows: Vec<_> = series
        .rows
        .iter()
        .map(|r| (r.date, r.soil_temp_5_f, r.air_min_f, r.air_avg_f, r.gdd50))
        .collect();
    assert_eq!(
        rows,
        vec![
            (d(9), Some(57.0), None, None, None),
            (d(10), Some(58.0), Some(60.0), Some(70.0), Some(20.0)),
            (d(11), None, Some(60.0), Some(70.0), Some(20.0)),
        ]
    );
}
