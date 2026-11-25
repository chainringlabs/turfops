//! A real app state for the timing handler and hooks: a migrated database holding the
//! weather record's lawn near Avondale, PA, years of 5 cm soil and daily air on a typical
//! year's curve, and optionally a forecast served by a local stub.

use crate::clock::Clock;
use crate::datasources::OpenWeatherMapClient;
use crate::db::queries;
use crate::db::weather_queries::days::start_of_day;
use crate::db::weather_queries::rows;
use crate::logic::data_sync::DataSyncService;
use crate::logic::weather_record::lab::{Lab, PRISM, STATION};
use crate::logic::weather_sync::forecast::ForecastSlot;
use crate::models::weather_record::{DailyRow, HourlyRow};
use crate::models::LawnProfile;
use crate::state::AppState;
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use sqlx::PgPool;

pub(super) const ZONE: Clock = Clock::Zone(chrono_tz::America::New_York);

/// The typical year (°F): peak 77 on Jul 19, trough 33 in mid-January.
fn curve(day: NaiveDate) -> f64 {
    55.0 + 22.0 * (std::f64::consts::TAU * (f64::from(day.ordinal()) - 200.0) / 365.0).cos()
}

/// The record's lawn (created by the Lab), with `years` complete years before `through`'s
/// year and that year up to `through`: the station's 5 cm soil on the curve (twelve
/// hours a day, enough for a covered day) and PRISM's daily air under it (mean 5 °F under,
/// minimum 16.5 °F under, maximum 6.5 °F over).
pub(crate) async fn climate(pool: &PgPool, years: i32, through: NaiveDate) -> Lab {
    let lab = Lab::new(pool.clone()).await;
    let first = NaiveDate::from_ymd_opt(through.year() - years, 1, 1).unwrap();
    let days: Vec<NaiveDate> = first.iter_days().take_while(|d| *d <= through).collect();
    let hours: Vec<HourlyRow> = days
        .iter()
        .flat_map(|day| {
            let start: DateTime<Utc> = start_of_day(chrono_tz::America::New_York, *day);
            let soil = curve(*day);
            (0..12).map(move |hour| {
                let mut row = HourlyRow {
                    observed_at: start + Duration::hours(hour),
                    ..Default::default()
                };
                row.soil_temp_f = [Some(soil), Some(soil), None, None, None];
                row
            })
        })
        .collect();
    let station = lab.source(STATION).await.id;
    let mut conn = lab.pool.acquire().await.unwrap();
    for chunk in hours.chunks(5000) {
        rows::write_hourly(&mut conn, station, chunk).await.unwrap();
    }
    let air: Vec<DailyRow> = days
        .iter()
        .map(|day| {
            let typical = curve(*day);
            DailyRow {
                day: *day,
                air_max_f: Some(typical + 6.5),
                air_min_f: Some(typical - 16.5),
                air_mean_f: Some(typical - 5.0),
                ..Default::default()
            }
        })
        .collect();
    let prism = lab.source(PRISM).await;
    for chunk in air.chunks(2000) {
        rows::write_daily(&mut conn, &prism, chunk).await.unwrap();
    }
    drop(conn);
    lab
}

/// App state over the database on `clock`, with no forecast.
pub(crate) async fn app_state_on(pool: &PgPool, clock: Clock) -> AppState {
    let mut config = crate::logic::data_sync::test_env::config(&[]);
    config.clock = clock;
    let sync = DataSyncService::initialize(&config, pool.clone()).await;
    AppState::new(pool.clone(), sync, None, false, clock)
}

/// App state on the lawn's clock, with the forecast fetched from `forecast_base` when
/// given.
pub(super) async fn app_state(pool: &PgPool, forecast_base: Option<&str>) -> AppState {
    let mut config = crate::logic::data_sync::test_env::config(&[]);
    config.clock = ZONE;
    let mut sync = DataSyncService::initialize(&config, pool.clone()).await;
    if let Some(base) = forecast_base {
        // The forecast as the schedule would hold it after fetching from `base`: none
        // when that fetch fails.
        let fetched = OpenWeatherMapClient::new()
            .with_base_url(base)
            .forecast(39.86, -75.79, "test-key", &ZONE, chrono::Utc::now())
            .await;
        if let Ok(forecast) = fetched {
            sync = sync.with_forecast(ForecastSlot::holding(forecast));
        }
    }
    AppState::new(pool.clone(), sync, None, false, ZONE)
}

/// The lawn profile as stored.
pub(crate) async fn lawn(pool: &PgPool) -> LawnProfile {
    queries::get_default_lawn_profile(pool)
        .await
        .unwrap()
        .unwrap()
}

/// Local instants for `day` at the given local hours.
fn at_hours(day: NaiveDate, hours: &[u32]) -> Vec<DateTime<Utc>> {
    let tz = chrono_tz::America::New_York;
    hours
        .iter()
        .filter_map(|&h| tz.from_local_datetime(&day.and_hms_opt(h, 0, 0)?).single())
        .map(|t| t.with_timezone(&Utc))
        .collect()
}
/// An OpenWeatherMap 5-day/3-hour body: `days` of `(offset from today, temps)` with one
/// 3-hourly point per temperature (8 points make a full day).
pub(super) fn forecast_body(days: &[(i64, Vec<f64>)]) -> &'static str {
    let today = ZONE.today();
    let mut items = Vec::new();
    for (offset, temps) in days {
        let hours: Vec<u32> = (0..temps.len() as u32).map(|i| 1 + 3 * i).collect();
        for (instant, temp) in at_hours(today + Duration::days(*offset), &hours)
            .into_iter()
            .zip(temps)
        {
            items.push(serde_json::json!({
                "dt": instant.timestamp(),
                "main": { "temp": temp, "feels_like": temp, "humidity": 60.0 },
                "weather": [{ "id": 800, "main": "Clear", "description": "clear sky" }],
                "clouds": { "all": 0.0 },
                "wind": { "speed": 5.0 },
                "pop": 0.0
            }));
        }
    }
    let body = serde_json::json!({
        "list": items,
        "city": {
            "name": "Avondale", "country": "US",
            "coord": { "lat": 39.86, "lon": -75.79 }, "timezone": -14400
        }
    });
    Box::leak(body.to_string().into_boxed_str())
}
