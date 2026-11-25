//! Gold daily contracts: the 10 cm and air/soil series and the GDD rows.

use super::fixtures::{self, GoldRow};
use chrono::{NaiveDate, TimeZone, Utc};

fn d(m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, m, day).unwrap()
}

/// Rows written out of date order: a full day, a sparse day, days missing a value.
fn days() -> Vec<GoldRow> {
    vec![
        GoldRow {
            soil_10_f: Some(61.0),
            air_avg_f: Some(66.0),
            ..GoldRow::day(d(9, 4))
        },
        GoldRow {
            soil_10_f: Some(60.0),
            air_avg_f: Some(65.0),
            ..GoldRow::day(d(9, 1))
        },
        GoldRow {
            hours_observed: 11,
            ..GoldRow::day(d(9, 2))
        },
        GoldRow {
            air_avg_f: None,
            soil_10_f: Some(59.0),
            ..GoldRow::day(d(9, 3))
        },
        GoldRow {
            soil_10_f: None,
            ..GoldRow::day(d(9, 5))
        },
    ]
}

// @spec WX-SRC-LAKE-007
#[tokio::test]
async fn gold_daily_series_keep_complete_days_in_date_order() {
    let gold = fixtures::gold(&days());
    let client = fixtures::client("unused.parquet", &gold.path);
    let (start, end) = (
        Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, 0).unwrap(),
        Utc.with_ymd_and_hms(2026, 9, 30, 0, 0, 0).unwrap(),
    );

    let soil: Vec<(NaiveDate, f64)> = client
        .fetch_daily_soil_temp_averages(start, end)
        .await
        .unwrap()
        .into_iter()
        .map(|s| (s.date, s.avg_temp_f))
        .collect();
    // Sep 2 has 11 hours; Sep 5 has no 10 cm mean.
    assert_eq!(
        soil,
        vec![(d(9, 1), 60.0), (d(9, 3), 59.0), (d(9, 4), 61.0)]
    );

    let pairs = client
        .fetch_daily_paired_averages(start, end)
        .await
        .unwrap();
    // Sep 3 also lacks air.
    assert_eq!(pairs, vec![(d(9, 1), 65.0, 60.0), (d(9, 4), 66.0, 61.0)]);
}
