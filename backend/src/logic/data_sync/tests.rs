use super::probes::*;
use super::sync_state::*;
use crate::error::TurfOpsError;
use crate::models::{
    DataSource, EnvironmentalReading, EnvironmentalSummary, ForecastProvider, Trend,
    WeatherForecast,
};
use chrono::{DateTime, Duration, Utc};

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn noon() -> DateTime<Utc> {
    at("2026-10-11T12:00:00Z")
}

fn lake(observed: DateTime<Utc>) -> EnvironmentalSummary {
    let mut current = EnvironmentalReading::new(DataSource::SoilData);
    current.timestamp = observed;
    current.soil_temp_10_f = Some(58.0);
    current.soil_moisture_10 = Some(0.25);
    current.precipitation_mm = Some(1.0);
    current.ambient_temp_f = Some(99.0); // station air: never the current reading's ambient
    EnvironmentalSummary {
        current: Some(current),
        soil_temp_7day_avg_f: Some(60.0),
        ambient_temp_7day_avg_f: Some(62.0),
        humidity_7day_avg: Some(70.0),
        precipitation_7day_total_mm: Some(12.0),
        soil_temp_trend: Trend::Falling,
        ..Default::default()
    }
}

fn patio(temp: Option<f64>, humidity: Option<f64>) -> EnvironmentalReading {
    let mut r = EnvironmentalReading::new(DataSource::HomeAssistant);
    r.ambient_temp_f = temp;
    r.humidity_percent = humidity;
    r
}

fn down<T>() -> crate::error::Result<T> {
    Err(TurfOpsError::Internal("source down".into()))
}

fn reads(
    lake: Option<crate::error::Result<EnvironmentalSummary>>,
    ha: Option<crate::error::Result<EnvironmentalReading>>,
) -> SensorReads {
    SensorReads {
        lake,
        home_assistant: ha,
        gdd: Some(Ok(Some(1500.0))),
    }
}

fn forecast(fetched_at: DateTime<Utc>) -> WeatherForecast {
    WeatherForecast {
        fetched_at,
        provider: ForecastProvider::OpenWeatherMap,
        hourly: vec![],
        daily_summary: vec![],
    }
}

fn soil(summary: &EnvironmentalSummary) -> Option<f64> {
    summary.current.as_ref().and_then(|c| c.soil_temp_10_f)
}

fn ambient(summary: &EnvironmentalSummary) -> (Option<f64>, Option<f64>) {
    let c = summary.current.as_ref().unwrap();
    (c.ambient_temp_f, c.humidity_percent)
}

// @spec SYNC-REFRESH-002, SYNC-REFRESH-008
#[test]
fn the_sensor_part_is_fresh_only_once_its_refresh_is_stored() {
    let mut state = SyncState::default();
    assert!(state.due(noon()), "never refreshed");
    // Reads gathered but not stored (the request was dropped): still due.
    let _abandoned = reads(Some(Ok(lake(noon()))), None);
    assert!(state.due(noon() + Duration::seconds(1)));

    state.store_sensors(noon(), reads(Some(Ok(lake(noon()))), None));
    assert!(!state.due(noon() + Duration::minutes(4)));
    assert!(state.due(noon() + Duration::minutes(5)));
}

// @spec SYNC-MERGE-001, SYNC-MERGE-002, SYNC-MERGE-003, SYNC-MERGE-004
#[test]
fn a_sensor_refresh_assembles_lake_soil_patio_air_and_gdd() {
    let mut state = SyncState::default();
    let observed = noon() - Duration::hours(20);
    state.store_sensors(
        noon(),
        reads(
            Some(Ok(lake(observed))),
            Some(Ok(patio(Some(71.0), Some(55.0)))),
        ),
    );
    let s = state.summary(noon(), None);
    assert_eq!(soil(&s), Some(58.0));
    assert_eq!(
        ambient(&s),
        (Some(71.0), Some(55.0)),
        "ambient comes from Home Assistant only"
    );
    assert_eq!(s.soil_observed_at, Some(observed));
    assert_eq!(s.current.as_ref().unwrap().timestamp, noon());
    assert_eq!(s.last_updated, Some(noon()));
    assert_eq!(s.gdd_base50_ytd, Some(1500.0));
    assert_eq!(s.soil_temp_7day_avg_f, Some(60.0));
}

// @spec SYNC-MERGE-006
#[test]
fn the_stale_soil_flag_follows_the_72_hour_limit() {
    let mut state = SyncState::default();
    state.store_sensors(
        noon(),
        reads(Some(Ok(lake(noon() - Duration::hours(71)))), None),
    );
    assert!(!state.summary(noon(), None).soil_stale);
    state.store_sensors(
        noon(),
        reads(Some(Ok(lake(noon() - Duration::hours(73)))), None),
    );
    assert!(state.summary(noon(), None).soil_stale);
    let mut empty = SyncState::default();
    empty.store_sensors(noon(), reads(None, None));
    assert!(
        !empty.summary(noon(), None).soil_stale,
        "no observation at all"
    );
}

// @spec SYNC-FAIL-002, SYNC-FAIL-003
#[test]
fn a_failed_lake_read_keeps_the_last_good_soil_and_gdd() {
    let mut state = SyncState::default();
    let observed = noon() - Duration::hours(20);
    state.store_sensors(noon(), reads(Some(Ok(lake(observed))), None));
    let later = noon() + Duration::minutes(6);
    state.store_sensors(
        later,
        SensorReads {
            lake: Some(down()),
            home_assistant: None,
            gdd: Some(down()),
        },
    );
    let s = state.summary(later, None);
    assert_eq!(soil(&s), Some(58.0));
    assert_eq!(s.soil_observed_at, Some(observed));
    assert_eq!(s.soil_temp_trend, Trend::Falling);
    assert_eq!(s.gdd_base50_ytd, Some(1500.0), "lake also failed: keep GDD");

    state.store_sensors(
        later,
        SensorReads {
            lake: Some(Ok(lake(observed))),
            home_assistant: None,
            gdd: Some(down()),
        },
    );
    assert_eq!(
        state.summary(later, None).gdd_base50_ytd,
        None,
        "lake read fine: GDD left empty"
    );
}

// @spec SYNC-FAIL-004, SYNC-FAIL-009
#[test]
fn soil_older_than_72_hours_drives_nothing() {
    let mut state = SyncState::default();
    let observed = noon() - Duration::hours(80);
    state.store_sensors(noon(), reads(Some(Ok(lake(observed))), None));
    let s = state.summary(noon(), None);
    let c = s.current.as_ref().unwrap();
    assert_eq!(
        (c.soil_temp_10_f, c.soil_moisture_10, c.precipitation_mm),
        (None, None, None)
    );
    assert_eq!(s.soil_observed_at, Some(observed), "still reported");
    assert_eq!(s.soil_temp_7day_avg_f, None);
    assert_eq!(s.ambient_temp_7day_avg_f, None);
    assert_eq!(s.humidity_7day_avg, None);
    assert_eq!(s.precipitation_7day_total_mm, None);
    assert_eq!(s.soil_temp_trend, Trend::Unknown);
}

// @spec SYNC-FAIL-004
#[test]
fn soil_carried_forward_past_72_hours_also_drives_nothing() {
    let mut state = SyncState::default();
    state.store_sensors(
        noon(),
        reads(Some(Ok(lake(noon() - Duration::hours(70)))), None),
    );
    let later = noon() + Duration::hours(3);
    state.store_sensors(later, reads(Some(down()), None));
    let s = state.summary(later, None);
    assert_eq!(soil(&s), None);
    assert!(s.soil_stale);
}

// @spec SYNC-FAIL-005
#[test]
fn a_failed_patio_read_holds_its_last_value_for_an_hour() {
    let mut state = SyncState::default();
    let lake_ok = || Some(Ok(lake(noon())));
    state.store_sensors(
        noon(),
        reads(lake_ok(), Some(Ok(patio(Some(71.0), Some(55.0))))),
    );
    let t1 = noon() + Duration::minutes(30);
    state.store_sensors(t1, reads(lake_ok(), Some(down())));
    assert_eq!(ambient(&state.summary(t1, None)), (Some(71.0), Some(55.0)));
    // A read that answers without humidity holds only the humidity.
    let t2 = noon() + Duration::minutes(40);
    state.store_sensors(t2, reads(lake_ok(), Some(Ok(patio(Some(73.0), None)))));
    assert_eq!(ambient(&state.summary(t2, None)), (Some(73.0), Some(55.0)));
    let t3 = noon() + Duration::minutes(61);
    state.store_sensors(t3, reads(lake_ok(), Some(down())));
    assert_eq!(
        ambient(&state.summary(t3, None)),
        (Some(73.0), None),
        "humidity read 61 min ago"
    );
    let mut unconfigured = SyncState::default();
    unconfigured.store_sensors(noon(), reads(lake_ok(), None));
    assert_eq!(ambient(&unconfigured.summary(noon(), None)), (None, None));
}

// @spec SYNC-FAIL-006, SYNC-FAIL-007
#[test]
fn a_forecast_is_served_until_it_is_six_hours_old() {
    let mut state = SyncState::default();
    state.store_sensors(noon(), reads(Some(Ok(lake(noon()))), None));
    let held = || Some(forecast(noon()));
    assert!(state.summary(noon(), held()).forecast.is_some());
    // The schedule's later fetches failed and it still holds the noon forecast.
    let later = noon() + Duration::hours(5) + Duration::minutes(59);
    assert!(
        state.summary(later, held()).forecast.is_some(),
        "under 6 h old: still served"
    );
    let six = noon() + Duration::hours(6);
    assert!(
        state.summary(six, held()).forecast.is_none(),
        "6 h old: never served"
    );
    assert!(state.summary(noon(), None).forecast.is_none());
}

#[test]
fn a_failed_sensor_read_is_retried_at_the_next_sensor_refresh() {
    let mut state = SyncState::default();
    state.store_sensors(noon(), reads(Some(down()), Some(down())));
    assert!(state.due(noon() + Duration::minutes(5)));
}

// @spec SYNC-CONN-006, MCP-SNAP-009
#[test]
fn a_probe_result_is_reused_for_five_minutes_up_or_down() {
    let mut cache = ProbeCache::default();
    assert_eq!(cache.recent(Probe::Lake, noon()), None);
    cache.record(Probe::Lake, false, noon());
    cache.record(Probe::HomeAssistant, true, noon());
    assert_eq!(
        cache.recent(Probe::Lake, noon() + Duration::minutes(4)),
        Some(false)
    );
    assert_eq!(
        cache.recent(Probe::HomeAssistant, noon() + Duration::minutes(4)),
        Some(true)
    );
    assert_eq!(
        cache.recent(Probe::Lake, noon() + Duration::minutes(5)),
        None
    );
    cache.record(Probe::Lake, true, noon() + Duration::minutes(5));
    assert_eq!(
        cache.recent(Probe::Lake, noon() + Duration::minutes(6)),
        Some(true)
    );
}
