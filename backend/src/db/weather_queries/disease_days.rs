//! An hourly source's local days as the disease models read them: only hours that have
//! both air temperature and humidity, in °C.

use super::days::start_of_day;
use crate::error::Result;
use crate::models::DailyWeather;
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use sqlx::{PgPool, Row};
use std::ops::RangeInclusive;

/// One day of a source, and its newest usable hour.
#[derive(Debug, Clone, PartialEq)]
pub struct DiseaseDay {
    pub weather: DailyWeather,
    pub newest_hour: DateTime<Utc>,
}

macro_rules! celsius {
    ($fahrenheit:literal) => {
        concat!("((", $fahrenheit, " - 32) * 5 / 9)")
    };
}

/// The Magnus term shared by the dew point's numerator and denominator.
macro_rules! gamma {
    () => {
        concat!(
            "(ln(rh_pct / 100) + 17.625 * ",
            celsius!("air_temp_f"),
            " / (243.04 + ",
            celsius!("air_temp_f"),
            "))"
        )
    };
}

const DAYS: &str = concat!(
    "SELECT (observed_at AT TIME ZONE $2)::date AS day, ",
    "count(*)::float8 AS hours, ",
    "avg(",
    celsius!("air_temp_f"),
    ") AS mean_c, ",
    "min(",
    celsius!("COALESCE(air_temp_min_f, air_temp_f)"),
    ") AS min_c, ",
    "max(",
    celsius!("COALESCE(air_temp_max_f, air_temp_f)"),
    ") AS max_c, ",
    "avg(rh_pct) AS rh, ",
    "(count(*) FILTER (WHERE rh_pct >= 90))::float8 AS rh90, ",
    "(count(*) FILTER (WHERE rh_pct >= 90 OR precip_mm > 0))::float8 AS wet, ",
    "max(243.04 * ",
    gamma!(),
    " / (17.625 - ",
    gamma!(),
    ")) AS dew_c, ",
    "bool_or(extract(hour FROM observed_at AT TIME ZONE $2) BETWEEN $5 AND $6) AS dawn, ",
    "bool_or(extract(hour FROM observed_at AT TIME ZONE $2) BETWEEN $7 AND $8) AS afternoon, ",
    "max(observed_at) AS newest ",
    "FROM weather_hourly ",
    "WHERE source_id = $1 AND observed_at >= $3 AND observed_at < $4 ",
    "AND air_temp_f IS NOT NULL AND rh_pct > 0 ",
    "GROUP BY 1 ORDER BY 1"
);

/// One hourly source's local days from `from` through `through`, ascending, counting
/// only hours with both air temperature and humidity. Rain is left at zero: the day's
/// total comes from the rain basis.
// @spec WX-REC-API-010
pub async fn disease_days(
    pool: &PgPool,
    source_id: i64,
    zone: Tz,
    span: (NaiveDate, NaiveDate),
    dawn: &RangeInclusive<u32>,
    afternoon: &RangeInclusive<u32>,
) -> Result<Vec<DiseaseDay>> {
    let until = span.1.succ_opt().unwrap_or(span.1);
    let rows = sqlx::query(DAYS)
        .bind(source_id)
        .bind(zone.name())
        .bind(start_of_day(zone, span.0))
        .bind(start_of_day(zone, until))
        .bind(*dawn.start() as i32)
        .bind(*dawn.end() as i32)
        .bind(*afternoon.start() as i32)
        .bind(*afternoon.end() as i32)
        .fetch_all(pool)
        .await?;
    rows.iter()
        .map(|row| {
            Ok(DiseaseDay {
                weather: DailyWeather {
                    date: row.try_get("day")?,
                    hours_covered: row.try_get("hours")?,
                    temp_mean_c: row.try_get("mean_c")?,
                    temp_min_c: row.try_get("min_c")?,
                    temp_max_c: row.try_get("max_c")?,
                    rh_mean: row.try_get("rh")?,
                    hours_rh90: row.try_get("rh90")?,
                    leaf_wetness_hours: row.try_get("wet")?,
                    precip_mm: 0.0,
                    dew_point_max_c: row.try_get("dew_c")?,
                    is_forecast: false,
                    covers_dawn: row.try_get("dawn")?,
                    covers_afternoon: row.try_get("afternoon")?,
                    basis: None,
                },
                newest_hour: row.try_get("newest")?,
            })
        })
        .collect::<sqlx::Result<Vec<_>>>()
        .map_err(Into::into)
}
