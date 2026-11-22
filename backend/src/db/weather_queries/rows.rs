//! Hourly and daily rows: written by replacing what is held, read back per source.

use crate::error::{Result, TurfOpsError};
use crate::models::weather_record::{DailyRow, HourlyRow, Source};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgConnection, PgPool};

macro_rules! hourly_columns {
    () => {
        "observed_at, air_temp_f, air_temp_max_f, air_temp_min_f, rh_pct, precip_mm, \
         soil_temp_5_f, soil_temp_10_f, soil_temp_20_f, soil_temp_50_f, soil_temp_100_f, \
         soil_moisture_5, soil_moisture_10, soil_moisture_20, soil_moisture_50, soil_moisture_100"
    };
}

/// Writes hourly rows for a source. A row already held for the same hour is replaced
/// whole, so a value the source has since withdrawn becomes empty here too.
// @spec WX-REC-STORE-002, WX-REC-STORE-003, WX-REC-STORE-004
pub async fn write_hourly(
    conn: &mut PgConnection,
    source_id: i64,
    rows: &[HourlyRow],
) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let column = |pick: fn(&HourlyRow) -> Option<f64>| -> Vec<Option<f64>> {
        rows.iter().map(pick).collect()
    };
    let times: Vec<DateTime<Utc>> = rows.iter().map(|r| r.observed_at).collect();
    let mut query = sqlx::query(
        r#"INSERT INTO weather_hourly (source_id, observed_at, air_temp_f, air_temp_max_f,
               air_temp_min_f, rh_pct, precip_mm, soil_temp_5_f, soil_temp_10_f, soil_temp_20_f,
               soil_temp_50_f, soil_temp_100_f, soil_moisture_5, soil_moisture_10,
               soil_moisture_20, soil_moisture_50, soil_moisture_100)
           SELECT $1, * FROM UNNEST($2::timestamptz[], $3::float8[], $4::float8[], $5::float8[],
               $6::float8[], $7::float8[], $8::float8[], $9::float8[], $10::float8[],
               $11::float8[], $12::float8[], $13::float8[], $14::float8[], $15::float8[],
               $16::float8[], $17::float8[])
           ON CONFLICT (source_id, observed_at) DO UPDATE SET
               air_temp_f = EXCLUDED.air_temp_f, air_temp_max_f = EXCLUDED.air_temp_max_f,
               air_temp_min_f = EXCLUDED.air_temp_min_f, rh_pct = EXCLUDED.rh_pct,
               precip_mm = EXCLUDED.precip_mm, soil_temp_5_f = EXCLUDED.soil_temp_5_f,
               soil_temp_10_f = EXCLUDED.soil_temp_10_f, soil_temp_20_f = EXCLUDED.soil_temp_20_f,
               soil_temp_50_f = EXCLUDED.soil_temp_50_f,
               soil_temp_100_f = EXCLUDED.soil_temp_100_f,
               soil_moisture_5 = EXCLUDED.soil_moisture_5,
               soil_moisture_10 = EXCLUDED.soil_moisture_10,
               soil_moisture_20 = EXCLUDED.soil_moisture_20,
               soil_moisture_50 = EXCLUDED.soil_moisture_50,
               soil_moisture_100 = EXCLUDED.soil_moisture_100"#,
    )
    .bind(source_id)
    .bind(times)
    .bind(column(|r| r.air_temp_f))
    .bind(column(|r| r.air_temp_max_f))
    .bind(column(|r| r.air_temp_min_f))
    .bind(column(|r| r.rh_pct))
    .bind(column(|r| r.precip_mm));
    for depth in 0..5 {
        query = query.bind(
            rows.iter()
                .map(|r| r.soil_temp_f[depth])
                .collect::<Vec<_>>(),
        );
    }
    for depth in 0..5 {
        query = query.bind(
            rows.iter()
                .map(|r| r.soil_moisture[depth])
                .collect::<Vec<_>>(),
        );
    }
    query.execute(conn).await?;
    Ok(())
}

/// Writes daily rows for a source that publishes daily values only. An hourly source's
/// days are computed from its hours on read, so it is refused here.
// @spec WX-REC-STORE-003, WX-REC-STORE-004, WX-REC-STORE-005
pub async fn write_daily(
    conn: &mut PgConnection,
    source: &Source,
    rows: &[DailyRow],
) -> Result<()> {
    if !source.publishes_daily() {
        return Err(TurfOpsError::Internal(format!(
            "weather source {} ({}) is hourly; its days are computed from its hours",
            source.id, source.key
        )));
    }
    if rows.is_empty() {
        return Ok(());
    }
    let column = |pick: fn(&DailyRow) -> Option<f64>| -> Vec<Option<f64>> {
        rows.iter().map(pick).collect()
    };
    sqlx::query(
        r#"INSERT INTO weather_daily
               (source_id, day, air_max_f, air_min_f, air_mean_f, precip_mm, soil_temp_5_f,
                soil_temp_10_f)
           SELECT $1, * FROM UNNEST($2::date[], $3::float8[], $4::float8[], $5::float8[],
               $6::float8[], $7::float8[], $8::float8[])
           ON CONFLICT (source_id, day) DO UPDATE SET
               air_max_f = EXCLUDED.air_max_f, air_min_f = EXCLUDED.air_min_f,
               air_mean_f = EXCLUDED.air_mean_f, precip_mm = EXCLUDED.precip_mm,
               soil_temp_5_f = EXCLUDED.soil_temp_5_f, soil_temp_10_f = EXCLUDED.soil_temp_10_f"#,
    )
    .bind(source.id)
    .bind(rows.iter().map(|r| r.day).collect::<Vec<NaiveDate>>())
    .bind(column(|r| r.air_max_f))
    .bind(column(|r| r.air_min_f))
    .bind(column(|r| r.air_mean_f))
    .bind(column(|r| r.precip_mm))
    .bind(column(|r| r.soil_temp_5_f))
    .bind(column(|r| r.soil_temp_10_f))
    .execute(conn)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct HourlyRecord {
    observed_at: DateTime<Utc>,
    air_temp_f: Option<f64>,
    air_temp_max_f: Option<f64>,
    air_temp_min_f: Option<f64>,
    rh_pct: Option<f64>,
    precip_mm: Option<f64>,
    soil_temp_5_f: Option<f64>,
    soil_temp_10_f: Option<f64>,
    soil_temp_20_f: Option<f64>,
    soil_temp_50_f: Option<f64>,
    soil_temp_100_f: Option<f64>,
    soil_moisture_5: Option<f64>,
    soil_moisture_10: Option<f64>,
    soil_moisture_20: Option<f64>,
    soil_moisture_50: Option<f64>,
    soil_moisture_100: Option<f64>,
}

impl From<HourlyRecord> for HourlyRow {
    fn from(r: HourlyRecord) -> Self {
        HourlyRow {
            observed_at: r.observed_at,
            air_temp_f: r.air_temp_f,
            air_temp_max_f: r.air_temp_max_f,
            air_temp_min_f: r.air_temp_min_f,
            rh_pct: r.rh_pct,
            precip_mm: r.precip_mm,
            soil_temp_f: [
                r.soil_temp_5_f,
                r.soil_temp_10_f,
                r.soil_temp_20_f,
                r.soil_temp_50_f,
                r.soil_temp_100_f,
            ],
            soil_moisture: [
                r.soil_moisture_5,
                r.soil_moisture_10,
                r.soil_moisture_20,
                r.soil_moisture_50,
                r.soil_moisture_100,
            ],
        }
    }
}

/// One source's hourly rows from `from` up to but not including `until`, ascending.
pub async fn hourly(
    pool: &PgPool,
    source_id: i64,
    from: DateTime<Utc>,
    until: DateTime<Utc>,
) -> Result<Vec<HourlyRow>> {
    let rows = sqlx::query_as::<_, HourlyRecord>(concat!(
        "SELECT ",
        hourly_columns!(),
        " FROM weather_hourly \
         WHERE source_id = $1 AND observed_at >= $2 AND observed_at < $3 ORDER BY observed_at"
    ))
    .bind(source_id)
    .bind(from)
    .bind(until)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(HourlyRow::from).collect())
}

/// One source's daily rows from `from` through `through`, ascending.
pub async fn daily(
    pool: &PgPool,
    source_id: i64,
    from: NaiveDate,
    through: NaiveDate,
) -> Result<Vec<DailyRow>> {
    let rows = sqlx::query_as::<
        _,
        (
            NaiveDate,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
        ),
    >(
        "SELECT day, air_max_f, air_min_f, air_mean_f, precip_mm, soil_temp_5_f, soil_temp_10_f \
         FROM weather_daily WHERE source_id = $1 AND day >= $2 AND day <= $3 ORDER BY day",
    )
    .bind(source_id)
    .bind(from)
    .bind(through)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(day, air_max_f, air_min_f, air_mean_f, precip_mm, soil_temp_5_f, soil_temp_10_f)| {
                DailyRow {
                    day,
                    air_max_f,
                    air_min_f,
                    air_mean_f,
                    precip_mm,
                    soil_temp_5_f,
                    soil_temp_10_f,
                }
            },
        )
        .collect())
}
