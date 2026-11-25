//! The first-run backfill as the long contracts see it.

use super::daily_db_tests::{air_lab, outage};
use super::lab::*;
use crate::error::Outage;
use chrono::NaiveDate;
use sqlx::PgPool;

// @spec WX-REC-FAIL-002, WX-REC-FAIL-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn long_contracts_wait_for_the_backfill_and_recent_ones_answer_from_what_is_held(
    pool: PgPool,
) {
    let lab = air_lab(pool).await;
    let jan1 = |year| NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    let hold = |key: &'static str, from: NaiveDate| {
        let lab = &lab;
        async move {
            let id = lab.source(key).await.id;
            sqlx::query(
                "UPDATE weather_fetch_state SET held_from = $2, held_through = $3 \
                 WHERE source_id = $1",
            )
            .bind(id)
            .bind(from)
            .bind(d(19))
            .execute(&lab.pool)
            .await
            .unwrap();
        }
    };
    hold(STATION, NaiveDate::from_ymd_opt(2019, 6, 1).unwrap()).await;

    let err = lab
        .record
        .fetch_climate_record(d(1), noon(20))
        .await
        .unwrap_err();
    assert!(
        err.owner_message()
            .contains("still loading (7 of 16 years)"),
        "{}",
        err.owner_message()
    );
    assert_eq!(
        outage(err),
        Outage::HistoryLoading {
            held: 7,
            wanted: 16
        }
    );
    let asked = noon(20);
    assert!(lab
        .record
        .fetch_daily_soil_temp_averages(d(1), d(20), asked)
        .await
        .is_err());
    assert!(lab
        .record
        .fetch_daily_paired_averages(d(1), d(20), asked)
        .await
        .is_err());
    assert_eq!(
        lab.record
            .fetch_daily_gdd(d(17), d(18), asked)
            .await
            .unwrap()
            .rows
            .len(),
        2
    );
    assert!(lab.record.fetch_gdd_ytd(asked).await.unwrap().is_some());

    hold(STATION, jan1(2011)).await;
    assert!(lab.record.fetch_climate_record(d(1), asked).await.is_ok());
}

// @spec WX-REC-FAIL-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn sources_wanted_from_different_years_are_loaded_once_each_holds_its_own(pool: PgPool) {
    let lab = air_lab(pool).await;
    // The estimate is wanted from 2012, a year after the station; each holds all of its own.
    let estimate = lab.source(ESTIMATE).await.id;
    let from_2012 = NaiveDate::from_ymd_opt(2012, 1, 1).unwrap();
    sqlx::query(
        "UPDATE weather_fetch_state SET wanted_from = $2, held_from = $2 WHERE source_id = $1",
    )
    .bind(estimate)
    .bind(from_2012)
    .execute(&lab.pool)
    .await
    .unwrap();

    let asked = noon(20);
    assert!(lab.record.fetch_climate_record(d(1), asked).await.is_ok());
    assert!(lab
        .record
        .fetch_daily_soil_temp_averages(d(1), d(20), asked)
        .await
        .is_ok());
    assert!(lab
        .record
        .fetch_daily_paired_averages(d(1), d(20), asked)
        .await
        .is_ok());
}
