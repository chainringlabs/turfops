//! The estimate where it fills a measured soil series, against PostgreSQL.

use super::lab::*;
use crate::models::weather_record::Input;
use sqlx::PgPool;

async fn soil(lab: &Lab, from: u32, through: u32, today: u32) -> Vec<(u32, f64)> {
    let series = lab
        .record
        .fetch_daily_soil_temp_averages(d(from), d(through), noon(today))
        .await
        .unwrap();
    series
        .rows
        .iter()
        .map(|row| (chrono::Datelike::day(&row.date), row.avg_temp_f))
        .collect()
}

/// The station reads 70 on the 10th to the 12th and 64 on the 16th and 17th; the
/// estimate is 60 on every day from the 5th to the 19th.
async fn holed(pool: PgPool) -> Lab {
    let lab = Lab::new(pool).await;
    for day in 10..=12 {
        lab.station_hours(day, 24, 70.0).await;
    }
    for day in 16..=17 {
        lab.station_hours(day, 24, 64.0).await;
    }
    for day in 5..=19 {
        lab.estimate(day, 60.0).await;
    }
    lab
}

// @spec WX-REC-RES-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_estimate_meets_the_measurements_on_both_sides_of_a_gap(pool: PgPool) {
    let lab = holed(pool).await;
    let got = soil(&lab, 5, 19, 20).await;
    assert_eq!(
        got,
        vec![
            (5, 60.0),
            (6, 60.0),
            (7, 60.0),
            (8, 60.0),
            (9, 60.0), // before the first measured day: the estimate as it is
            (10, 70.0),
            (11, 70.0),
            (12, 70.0),
            (13, 68.5), // from +10 at one edge
            (14, 67.0),
            (15, 65.5), // to +4 at the other
            (16, 64.0),
            (17, 64.0),
        ]
    );
}

// @spec WX-REC-RES-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_day_is_anchored_the_same_whatever_span_is_asked_for(pool: PgPool) {
    let lab = holed(pool).await;
    assert_eq!(soil(&lab, 14, 14, 20).await, vec![(14, 67.0)]);
    assert_eq!(soil(&lab, 15, 16, 20).await, vec![(15, 65.5), (16, 64.0)]);
}

// @spec WX-REC-RES-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn after_a_station_goes_quiet_the_estimate_starts_from_where_it_left_off(pool: PgPool) {
    let lab = Lab::new(pool).await;
    for day in 10..=17 {
        lab.station_hours(day, 24, 64.0).await;
    }
    for day in 5..=21 {
        lab.estimate(day, 60.0).await;
    }
    // Four days after the station's last hour it is past its age limit.
    let got = soil(&lab, 17, 21, 22).await;
    assert_eq!(
        got,
        vec![(17, 64.0), (18, 63.9), (19, 63.7), (20, 63.6), (21, 63.5)],
        "the +4 °F offset loses a thirtieth a day"
    );
    // The live reading is the estimate's newest day, met the same way.
    let reading = lab.record.fetch_latest(noon(22)).await.unwrap().unwrap();
    let live = reading.get(Input::SoilTemp10).unwrap();
    assert_eq!(live.value, 63.5);
    assert_eq!(live.basis.source, "Soil temperature estimate");
}

// @spec WX-REC-RES-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn an_estimate_with_no_measured_series_beside_it_is_left_as_it_is(pool: PgPool) {
    let lab = Lab::new(pool).await;
    for day in 10..=19 {
        lab.estimate(day, 60.0).await;
    }
    assert!(soil(&lab, 10, 19, 20)
        .await
        .iter()
        .all(|(_, value)| *value == 60.0));
}
