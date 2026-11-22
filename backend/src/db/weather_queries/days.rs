//! An hourly source's hours grouped into the lawn's local days, and how recently each
//! source reported. Computed on read: no daily copy of an hourly source is stored.

use crate::error::Result;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use sqlx::{PgPool, Row};

/// The valid hours of one value on one day, and their mean.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stat {
    pub hours: i64,
    pub mean: Option<f64>,
}

/// One lawn-local day of an hourly source.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HourlyDay {
    pub day: NaiveDate,
    pub air: Stat,
    pub air_max: Option<f64>,
    pub air_min: Option<f64>,
    pub humidity: Stat,
    pub rain_hours: i64,
    pub rain_mm: Option<f64>,
    /// At 5, 10, 20, 50 and 100 cm.
    pub soil_temp: [Stat; 5],
    pub soil_moisture: [Stat; 5],
}

macro_rules! stat {
    ($column:literal, $name:literal) => {
        concat!("count(", $column, ") AS ", $name, "_n, avg(", $column, ") AS ", $name, ", ")
    };
}

const DAYS: &str = concat!(
    "SELECT (observed_at AT TIME ZONE $2)::date AS day, ",
    stat!("air_temp_f", "air"),
    "max(COALESCE(air_temp_max_f, air_temp_f)) AS air_max, ",
    "min(COALESCE(air_temp_min_f, air_temp_f)) AS air_min, ",
    stat!("rh_pct", "rh"),
    "count(precip_mm) AS rain_n, sum(precip_mm) AS rain, ",
    stat!("soil_temp_5_f", "st5"),
    stat!("soil_temp_10_f", "st10"),
    stat!("soil_temp_20_f", "st20"),
    stat!("soil_temp_50_f", "st50"),
    stat!("soil_temp_100_f", "st100"),
    stat!("soil_moisture_5", "sm5"),
    stat!("soil_moisture_10", "sm10"),
    stat!("soil_moisture_20", "sm20"),
    stat!("soil_moisture_50", "sm50"),
    "count(soil_moisture_100) AS sm100_n, avg(soil_moisture_100) AS sm100 ",
    "FROM weather_hourly \
     WHERE source_id = $1 AND observed_at >= $3 AND observed_at < $4 \
     GROUP BY 1 ORDER BY 1"
);

/// The instant a local day starts in `zone`.
pub fn start_of_day(zone: Tz, day: NaiveDate) -> DateTime<Utc> {
    let midnight = day.and_hms_opt(0, 0, 0).expect("midnight exists");
    zone.from_local_datetime(&midnight)
        .earliest()
        // A zone that skips midnight starts its day at the first hour that exists.
        .unwrap_or_else(|| zone.from_utc_datetime(&midnight))
        .with_timezone(&Utc)
}

/// One hourly source's local days from `from` through `through`, ascending. A day with
/// no hours has no entry.
// @spec WX-REC-STORE-005
pub async fn local_days(
    pool: &PgPool,
    source_id: i64,
    zone: Tz,
    from: NaiveDate,
    through: NaiveDate,
) -> Result<Vec<HourlyDay>> {
    let until = through.succ_opt().unwrap_or(through);
    let rows = sqlx::query(DAYS)
        .bind(source_id)
        .bind(zone.name())
        .bind(start_of_day(zone, from))
        .bind(start_of_day(zone, until))
        .fetch_all(pool)
        .await?;
    rows.iter()
        .map(|row| {
            let stat = |name: &str| -> sqlx::Result<Stat> {
                Ok(Stat {
                    hours: row.try_get(format!("{name}_n").as_str())?,
                    mean: row.try_get(name)?,
                })
            };
            Ok(HourlyDay {
                day: row.try_get("day")?,
                air: stat("air")?,
                air_max: row.try_get("air_max")?,
                air_min: row.try_get("air_min")?,
                humidity: stat("rh")?,
                rain_hours: row.try_get("rain_n")?,
                rain_mm: row.try_get("rain")?,
                soil_temp: [
                    stat("st5")?,
                    stat("st10")?,
                    stat("st20")?,
                    stat("st50")?,
                    stat("st100")?,
                ],
                soil_moisture: [
                    stat("sm5")?,
                    stat("sm10")?,
                    stat("sm20")?,
                    stat("sm50")?,
                    stat("sm100")?,
                ],
            })
        })
        .collect::<sqlx::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// When an hourly source last gave each of its values, looking back to `since`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewestHours {
    pub air: Option<DateTime<Utc>>,
    pub humidity: Option<DateTime<Utc>>,
    pub rain: Option<DateTime<Utc>>,
    pub soil_temp: [Option<DateTime<Utc>>; 5],
    pub soil_moisture: [Option<DateTime<Utc>>; 5],
}

macro_rules! newest {
    ($column:literal) => {
        concat!("max(observed_at) FILTER (WHERE ", $column, " IS NOT NULL)")
    };
}

const NEWEST: &str = concat!(
    "SELECT ",
    newest!("air_temp_f"),
    ", ",
    newest!("rh_pct"),
    ", ",
    newest!("precip_mm"),
    ", ",
    newest!("soil_temp_5_f"),
    ", ",
    newest!("soil_temp_10_f"),
    ", ",
    newest!("soil_temp_20_f"),
    ", ",
    newest!("soil_temp_50_f"),
    ", ",
    newest!("soil_temp_100_f"),
    ", ",
    newest!("soil_moisture_5"),
    ", ",
    newest!("soil_moisture_10"),
    ", ",
    newest!("soil_moisture_20"),
    ", ",
    newest!("soil_moisture_50"),
    ", ",
    newest!("soil_moisture_100"),
    " FROM weather_hourly WHERE source_id = $1 AND observed_at >= $2"
);

/// The start of the newest hour holding each value, among hours from `since`.
pub async fn newest_hours(
    pool: &PgPool,
    source_id: i64,
    since: DateTime<Utc>,
) -> Result<NewestHours> {
    let row = sqlx::query(NEWEST)
        .bind(source_id)
        .bind(since)
        .fetch_one(pool)
        .await?;
    let at = |index: usize| row.try_get::<Option<DateTime<Utc>>, _>(index);
    Ok(NewestHours {
        air: at(0)?,
        humidity: at(1)?,
        rain: at(2)?,
        soil_temp: [at(3)?, at(4)?, at(5)?, at(6)?, at(7)?],
        soil_moisture: [at(8)?, at(9)?, at(10)?, at(11)?, at(12)?],
    })
}
