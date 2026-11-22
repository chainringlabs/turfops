//! A lawn near Avondale, PA with a stored plan, for tests of the record's contracts.
//! "Now" is noon Eastern on 2026-07-20 unless a test says otherwise.

use super::plan::{build, Lawn, MappedSensor};
use super::record::{AgeLimits, Asked, WeatherRecord};
use crate::clock::Moment;
use crate::db::weather_queries::{catalog, days, fetch_state, plan, rows, sources};
use crate::models::weather_record::*;
use crate::models::{GrassType, LawnLocation};
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::America::New_York;
use sqlx::PgPool;

pub const STATION: &str = "station:Uscrn:a";
pub const ESTIMATE: &str = "estimate:39.8600,-75.7900";
pub const PRISM: &str = "grid:prism:39.8600,-75.7900";
pub const NRCC: &str = "grid:nrcc:39.8600,-75.7900";
pub const ANALYSIS: &str = "analysis:39.8600,-75.7900";
pub const SENSOR: &str = "sensor:1";

const LAWN: Lawn = Lawn {
    latitude: 39.86,
    longitude: -75.79,
    elevation_ft: Some(400.0),
};

pub const LIMITS: AgeLimits = AgeLimits {
    sensor_air: Duration::hours(1),
    sensor_soil: Duration::hours(6),
    analysis: Duration::hours(3),
    station: Duration::hours(72),
    daily: Duration::hours(72),
};

pub fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 7, day).unwrap()
}

/// Noon Eastern on July `day`.
pub fn noon(day: u32) -> Asked {
    let now = Utc.with_ymd_and_hms(2026, 7, day, 16, 0, 0).unwrap();
    Asked {
        at: Moment { now, today: d(day) },
        limits: LIMITS,
    }
}

pub struct Lab {
    pub pool: PgPool,
    pub record: WeatherRecord,
}

impl Lab {
    /// A lawn with a location and a plan: one USCRN station 20 miles off that reports
    /// soil temperature, and the estimates.
    pub async fn new(pool: PgPool) -> Self {
        let mut profile = crate::logic::rules::test_fixtures::profile(GrassType::TallFescue);
        profile.id = None;
        profile.location = Some(LawnLocation::at(39.86, -75.79, None).unwrap());
        crate::db::queries::create_lawn_profile(&pool, &profile)
            .await
            .unwrap();
        let station = Station {
            network: Network::Uscrn,
            station_id: "a".into(),
            name: "Avondale 2 N".into(),
            state: "PA".into(),
            latitude: 39.86 + 20.0 / 69.09,
            longitude: -75.79,
            elevation_ft: Some(400.0),
            inputs: vec![Input::SoilTemp5, Input::SoilTemp10],
            first_year: Some(2006),
            last_year: None,
            locator: None,
            utc_offset_hours: None,
        };
        catalog::replace_network(&pool, Network::Uscrn, &[station])
            .await
            .unwrap();
        let stations = catalog::stations(&pool).await.unwrap();
        plan::replace(&pool, &build(Some(&LAWN), &stations, &[], &[]))
            .await
            .unwrap();
        // Every source starts with its history in hand; a test of the backfill undoes it.
        for source in sources::all(&pool).await.unwrap() {
            let from = NaiveDate::from_ymd_opt(2011, 1, 1).unwrap();
            fetch_state::want(&pool, source.id, "history", from)
                .await
                .unwrap();
            sqlx::query(
                "UPDATE weather_fetch_state SET held_from = wanted_from, held_through = $2 \
                 WHERE source_id = $1",
            )
            .bind(source.id)
            .bind(d(19))
            .execute(&pool)
            .await
            .unwrap();
        }
        let record = WeatherRecord::new(pool.clone());
        Self { pool, record }
    }

    pub async fn source(&self, key: &str) -> Source {
        let all = sources::all(&self.pool).await.unwrap();
        all.into_iter().find(|s| s.key == key).expect(key)
    }

    /// The first `hours` hours of local July `day` from the station, soil at `soil_f`.
    pub async fn station_hours(&self, day: u32, hours: i64, soil_f: f64) {
        let start: DateTime<Utc> = days::start_of_day(New_York, d(day));
        let rows: Vec<HourlyRow> = (0..hours)
            .map(|hour| {
                let mut row = HourlyRow {
                    observed_at: start + Duration::hours(hour),
                    ..Default::default()
                };
                row.soil_temp_f = [Some(soil_f + 2.0), Some(soil_f), None, None, None];
                row
            })
            .collect();
        let id = self.source(STATION).await.id;
        let mut conn = self.pool.acquire().await.unwrap();
        rows::write_hourly(&mut conn, id, &rows).await.unwrap();
    }

    /// The first `hours` hours of local July `day` from the hourly analysis.
    pub async fn analysis_hours(&self, day: u32, hours: i64, air_f: f64, rh: f64, rain_mm: f64) {
        let start: DateTime<Utc> = days::start_of_day(New_York, d(day));
        let rows: Vec<HourlyRow> = (0..hours)
            .map(|hour| HourlyRow {
                observed_at: start + Duration::hours(hour),
                air_temp_f: Some(air_f),
                rh_pct: Some(rh),
                precip_mm: Some(rain_mm),
                ..Default::default()
            })
            .collect();
        self.write_hours(ANALYSIS, &rows).await;
    }

    pub async fn write_hours(&self, key: &str, rows: &[HourlyRow]) {
        let id = self.source(key).await.id;
        let mut conn = self.pool.acquire().await.unwrap();
        rows::write_hourly(&mut conn, id, rows).await.unwrap();
    }

    /// Rebuilds the plan with one of the owner's sensors mapped to `input`.
    pub async fn map_sensor(&self, input: Input) -> Source {
        let stations = catalog::stations(&self.pool).await.unwrap();
        let sensor = MappedSensor {
            input,
            key: SENSOR.into(),
            name: "Patio".into(),
        };
        plan::replace(&self.pool, &build(Some(&LAWN), &stations, &[sensor], &[]))
            .await
            .unwrap();
        self.source(SENSOR).await
    }

    /// One of a daily source's own days.
    pub async fn daily(&self, key: &str, row: DailyRow) {
        let source = self.source(key).await;
        let mut conn = self.pool.acquire().await.unwrap();
        rows::write_daily(&mut conn, &source, &[row]).await.unwrap();
    }

    pub async fn estimate(&self, day: u32, soil_f: f64) {
        let row = DailyRow {
            day: d(day),
            soil_temp_5_f: Some(soil_f + 2.0),
            soil_temp_10_f: Some(soil_f),
            ..Default::default()
        };
        self.daily(ESTIMATE, row).await;
    }

    pub async fn grid(&self, key: &str, day: u32, max_f: f64, min_f: f64) {
        let row = DailyRow {
            day: d(day),
            air_max_f: Some(max_f),
            air_min_f: Some(min_f),
            air_mean_f: Some((max_f + min_f) / 2.0),
            ..Default::default()
        };
        self.daily(key, row).await;
    }
}
