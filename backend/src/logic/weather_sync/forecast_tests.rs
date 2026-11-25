//! When the forecast is fetched, what each fetch leaves held, and the status it reports.

use super::*;
use chrono::{Duration, TimeZone};

const MINUTE: u32 = 7;

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 24, hour, minute, 0).unwrap()
}

fn choice(provider: ForecastProvider) -> Choice {
    Choice {
        provider,
        key: None,
    }
}

fn owm(key: &str) -> Choice {
    Choice {
        provider: ForecastProvider::OpenWeatherMap,
        key: Some(key.into()),
    }
}

fn forecast(provider: ForecastProvider, fetched_at: DateTime<Utc>) -> WeatherForecast {
    WeatherForecast {
        fetched_at,
        provider,
        hourly: Vec::new(),
        daily_summary: Vec::new(),
    }
}

fn fetched(provider: ForecastProvider, now: DateTime<Utc>) -> Outcome {
    Outcome::Fetched(forecast(provider, now))
}

const DOWN: &str = "The weather forecast is unavailable right now.";

fn failed() -> Outcome {
    Outcome::Failed(DOWN.into())
}

/// A keeper that fetched from Open-Meteo at 10:07.
fn kept_at_ten() -> ForecastKeeper {
    let mut keeper = ForecastKeeper::new(ForecastSlot::new());
    let open_meteo = choice(ForecastProvider::OpenMeteo);
    keeper.keep(
        &open_meteo,
        fetched(ForecastProvider::OpenMeteo, at(10, 7)),
        at(10, 7),
    );
    keeper
}

// @spec SYNC-SCHED-019
#[test]
fn the_first_turn_after_a_start_is_due_whatever_the_minute() {
    let keeper = ForecastKeeper::new(ForecastSlot::new());
    assert!(keeper.due(&choice(ForecastProvider::OpenMeteo), at(10, 42), MINUTE));
}

// @spec SYNC-SCHED-019
#[test]
fn then_the_forecast_is_due_hourly_at_the_installations_minute() {
    let keeper = kept_at_ten();
    let open_meteo = choice(ForecastProvider::OpenMeteo);
    assert!(!keeper.due(&open_meteo, at(10, 30), MINUTE));
    assert!(!keeper.due(&open_meteo, at(11, 6), MINUTE));
    assert!(keeper.due(&open_meteo, at(11, 7), MINUTE));
    assert!(keeper.due(&open_meteo, at(13, 50), MINUTE), "a missed hour");
}

// @spec SYNC-SCHED-019
#[test]
fn a_fetch_off_the_minute_is_followed_by_one_at_the_next_minute() {
    // A start at 10:42 fetches at once; the next is 11:07, not 11:42.
    let mut keeper = ForecastKeeper::new(ForecastSlot::new());
    let open_meteo = choice(ForecastProvider::OpenMeteo);
    keeper.keep(
        &open_meteo,
        fetched(ForecastProvider::OpenMeteo, at(10, 42)),
        at(10, 42),
    );
    assert!(!keeper.due(&open_meteo, at(11, 6), MINUTE));
    assert!(keeper.due(&open_meteo, at(11, 7), MINUTE));
}

// @spec SYNC-SCHED-019
#[test]
fn a_change_of_provider_or_key_is_due_at_once() {
    let keeper = kept_at_ten();
    assert!(keeper.due(
        &choice(ForecastProvider::NationalWeatherService),
        at(10, 9),
        MINUTE
    ));

    let mut keeper = ForecastKeeper::new(ForecastSlot::new());
    keeper.keep(
        &owm("old"),
        fetched(ForecastProvider::OpenWeatherMap, at(10, 7)),
        at(10, 7),
    );
    assert!(!keeper.due(&owm("old"), at(10, 9), MINUTE));
    assert!(keeper.due(&owm("new"), at(10, 9), MINUTE));
}

// @spec SYNC-SCHED-022
#[test]
fn a_failed_fetch_is_tried_again_after_5_then_15_minutes_then_hourly() {
    let mut keeper = kept_at_ten();
    let open_meteo = choice(ForecastProvider::OpenMeteo);
    keeper.keep(&open_meteo, failed(), at(11, 7));
    assert!(!keeper.due(&open_meteo, at(11, 11), MINUTE));
    assert!(keeper.due(&open_meteo, at(11, 12), MINUTE));

    keeper.keep(&open_meteo, failed(), at(11, 12));
    assert!(!keeper.due(&open_meteo, at(11, 26), MINUTE));
    assert!(keeper.due(&open_meteo, at(11, 27), MINUTE));

    keeper.keep(&open_meteo, failed(), at(11, 27));
    assert!(!keeper.due(&open_meteo, at(12, 26), MINUTE));
    assert!(keeper.due(&open_meteo, at(12, 27), MINUTE));

    keeper.keep(
        &open_meteo,
        fetched(ForecastProvider::OpenMeteo, at(12, 27)),
        at(12, 27),
    );
    assert!(
        !keeper.due(&open_meteo, at(13, 6), MINUTE),
        "back on the hour"
    );
    assert!(keeper.due(&open_meteo, at(13, 7), MINUTE));
}

// @spec SYNC-SCHED-020, SYNC-CONN-007
#[test]
fn a_fetch_replaces_the_forecast_held_and_clears_the_failure() {
    let mut keeper = kept_at_ten();
    let open_meteo = choice(ForecastProvider::OpenMeteo);
    keeper.keep(&open_meteo, failed(), at(11, 7));
    keeper.keep(
        &open_meteo,
        fetched(ForecastProvider::OpenMeteo, at(11, 12)),
        at(11, 12),
    );
    let slot = keeper.slot();
    assert_eq!(slot.forecast().unwrap().fetched_at, at(11, 12));
    assert_eq!(
        slot.status(ForecastProvider::OpenMeteo),
        ForecastStatus {
            provider: ForecastProvider::OpenMeteo,
            fetched_at: Some(at(11, 12)),
            last_error: None,
        }
    );
}

// @spec SYNC-SCHED-022, SYNC-CONN-007
#[test]
fn a_failure_keeps_the_forecast_held_and_says_why() {
    let mut keeper = kept_at_ten();
    keeper.keep(&choice(ForecastProvider::OpenMeteo), failed(), at(11, 7));
    let slot = keeper.slot();
    assert_eq!(slot.forecast().unwrap().fetched_at, at(10, 7));
    assert_eq!(
        slot.status(ForecastProvider::OpenMeteo),
        ForecastStatus {
            provider: ForecastProvider::OpenMeteo,
            fetched_at: Some(at(10, 7)),
            last_error: Some(DOWN.into()),
        }
    );
}

// @spec SYNC-SCHED-020, SYNC-CONN-007
#[test]
fn a_change_of_provider_discards_the_forecast_held() {
    let mut keeper = kept_at_ten();
    let nws = choice(ForecastProvider::NationalWeatherService);
    keeper.keep(&nws, failed(), at(10, 9));
    let slot = keeper.slot();
    assert!(slot.forecast().is_none());
    assert_eq!(
        slot.status(ForecastProvider::NationalWeatherService),
        ForecastStatus {
            provider: ForecastProvider::NationalWeatherService,
            fetched_at: None,
            last_error: Some(DOWN.into()),
        }
    );
}

// @spec SYNC-CONN-007
#[test]
fn a_provider_not_yet_fetched_for_reports_nothing_fetched_and_no_failure() {
    let empty = ForecastSlot::new();
    assert_eq!(
        empty.status(ForecastProvider::OpenMeteo),
        ForecastStatus {
            provider: ForecastProvider::OpenMeteo,
            fetched_at: None,
            last_error: None,
        }
    );
    // Chosen a moment ago; the schedule has not had its turn yet.
    let slot = kept_at_ten().slot();
    assert_eq!(
        slot.status(ForecastProvider::NationalWeatherService),
        ForecastStatus {
            provider: ForecastProvider::NationalWeatherService,
            fetched_at: None,
            last_error: None,
        }
    );
}

// @spec SYNC-SCHED-023, WX-SRC-FC-004
#[test]
fn a_provider_that_needs_its_key_says_so_and_holds_no_forecast() {
    let mut keeper = kept_at_ten();
    let keyless = choice(ForecastProvider::OpenWeatherMap);
    let logged = keeper.keep(&keyless, Outcome::NeedsKey, at(10, 9));
    assert_eq!(logged, Logged::Warn);
    let status = keeper.slot().status(ForecastProvider::OpenWeatherMap);
    assert_eq!(status.fetched_at, None);
    assert_eq!(
        status.last_error,
        Some(needs_key(ForecastProvider::OpenWeatherMap))
    );
    assert!(keeper.slot().forecast().is_none());
    let sentence = needs_key(ForecastProvider::OpenWeatherMap);
    assert!(sentence.contains("OpenWeatherMap"), "{sentence}");
    assert!(sentence.contains("key"), "{sentence}");
}

// @spec SYNC-SCHED-021
#[test]
fn the_first_forecast_after_a_start_a_failure_or_a_change_is_logged_at_info() {
    let mut keeper = ForecastKeeper::new(ForecastSlot::new());
    let open_meteo = choice(ForecastProvider::OpenMeteo);
    let fetch = |keeper: &mut ForecastKeeper, choice: &Choice, now| {
        keeper.keep(choice, fetched(choice.provider, now), now)
    };
    assert_eq!(
        fetch(&mut keeper, &open_meteo, at(10, 7)),
        Logged::Info,
        "start"
    );
    assert_eq!(fetch(&mut keeper, &open_meteo, at(11, 7)), Logged::Debug);

    assert_eq!(keeper.keep(&open_meteo, failed(), at(12, 7)), Logged::Warn);
    assert_eq!(
        fetch(&mut keeper, &open_meteo, at(12, 12)),
        Logged::Info,
        "after a failure"
    );
    assert_eq!(fetch(&mut keeper, &open_meteo, at(13, 7)), Logged::Debug);

    let nws = choice(ForecastProvider::NationalWeatherService);
    assert_eq!(
        fetch(&mut keeper, &nws, at(13, 9)),
        Logged::Info,
        "a change of provider"
    );
    assert_eq!(fetch(&mut keeper, &nws, at(14, 7)), Logged::Debug);
}

// @spec SYNC-SCHED-020
#[test]
fn a_held_forecast_is_read_however_old() {
    // The summary, not the slot, drops a forecast at 6 hours (SYNC-FAIL-007).
    let slot = ForecastSlot::holding(forecast(
        ForecastProvider::OpenMeteo,
        at(10, 7) - Duration::hours(30),
    ));
    assert!(slot.forecast().is_some());
    assert_eq!(
        slot.status(ForecastProvider::OpenMeteo).fetched_at,
        Some(at(10, 7) - Duration::hours(30))
    );
}

#[test]
fn a_choice_never_prints_its_key() {
    let printed = format!("{:?}", owm("sk-secret-123"));
    assert!(!printed.contains("sk-secret-123"), "{printed}");
}

// @spec SYNC-SCHED-009
#[test]
fn a_forced_turn_is_due_whatever_the_hour_or_a_failure() {
    let mut keeper = kept_at_ten();
    let open_meteo = choice(ForecastProvider::OpenMeteo);
    assert!(!keeper.due(&open_meteo, at(10, 20), MINUTE));
    keeper.force();
    assert!(keeper.due(&open_meteo, at(10, 20), MINUTE));
    keeper.keep(
        &open_meteo,
        fetched(ForecastProvider::OpenMeteo, at(10, 20)),
        at(10, 20),
    );
    assert!(!keeper.due(&open_meteo, at(10, 21), MINUTE), "once");

    keeper.keep(&open_meteo, failed(), at(10, 30));
    assert!(!keeper.due(&open_meteo, at(10, 31), MINUTE));
    keeper.force();
    assert!(
        keeper.due(&open_meteo, at(10, 31), MINUTE),
        "the retry wait too"
    );
}
