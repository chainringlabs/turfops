//! Judging one Home Assistant answer for a mapped sensor.

use super::*;
use crate::datasources::homeassistant::connection::Entity;
use chrono::{Duration, TimeZone};

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 24, 15, 0, 0).unwrap()
}

fn entity(value: f64, unit: &str, minutes_ago: i64) -> Lookup {
    Lookup::Reading(Entity {
        entity_id: "sensor.probe".into(),
        friendly_name: "Probe".into(),
        value,
        unit: (!unit.is_empty()).then(|| unit.to_string()),
        device_class: None,
        state_class: None,
        reported_at: now() - Duration::minutes(minutes_ago),
    })
}

fn previous(value: f64, minutes_ago: i64) -> Option<Previous> {
    Some(Previous {
        value,
        at: now() - Duration::minutes(minutes_ago),
    })
}

fn stored(judged: Judged) -> (DateTime<Utc>, f64, Previous, bool) {
    match judged {
        Judged::Store {
            at,
            value,
            keep,
            jumped,
        } => (at, value, keep, jumped),
        other => panic!("expected a reading to store, got {other:?}"),
    }
}

// @spec WX-SRC-HA-001
#[test]
fn a_reading_is_the_value_in_record_units_at_its_reported_time() {
    let judged = judge(Input::AirTemp, None, entity(20.0, "°C", 3), None, now());
    let (at, value, keep, jumped) = stored(judged);
    assert_eq!(at, now() - Duration::minutes(3));
    assert!((value - 68.0).abs() < 1e-9);
    assert_eq!(keep, Previous { value, at });
    assert!(!jumped);
}

// @spec WX-SRC-HA-003
#[test]
fn a_reported_time_not_later_than_the_previous_reading_is_nothing_new() {
    let same = judge(
        Input::Humidity,
        None,
        entity(70.0, "%", 10),
        previous(65.0, 10),
        now(),
    );
    assert_eq!(same, Judged::Unchanged);
    let earlier = judge(
        Input::Humidity,
        None,
        entity(70.0, "%", 30),
        previous(65.0, 10),
        now(),
    );
    assert_eq!(
        earlier,
        Judged::Unchanged,
        "a clock set back writes nothing"
    );
    let later = judge(
        Input::Humidity,
        None,
        entity(70.0, "%", 5),
        previous(65.0, 10),
        now(),
    );
    assert_eq!(stored(later).1, 70.0);
}

// @spec WX-SRC-HA-002, WX-SRC-HA-013
#[test]
fn a_state_that_is_not_a_number_or_an_entity_that_is_gone_is_no_reading() {
    let gone = judge(
        Input::AirTemp,
        None,
        Lookup::Missing,
        previous(60.0, 10),
        now(),
    );
    assert_eq!(gone, Judged::Refused(Problem::NotFound));
    let off = judge(Input::AirTemp, None, Lookup::NotANumber, None, now());
    assert_eq!(off, Judged::Refused(Problem::NotANumber));
}

// @spec WX-SRC-HA-008
#[test]
fn a_unit_the_input_does_not_accept_or_an_implausible_value_is_no_reading() {
    let kelvin = judge(Input::AirTemp, None, entity(290.0, "K", 1), None, now());
    assert_eq!(kelvin, Judged::Refused(Problem::Unit(Some("K".into()))));
    let hot = judge(Input::AirTemp, None, entity(200.0, "°F", 1), None, now());
    assert_eq!(hot, Judged::Refused(Problem::Implausible(200.0)));
    let wet = judge(
        Input::SoilMoisture10,
        None,
        entity(140.0, "%", 1),
        None,
        now(),
    );
    assert_eq!(wet, Judged::Refused(Problem::Implausible(1.4)));
}

// @spec WX-SRC-HA-008, WX-SRC-HA-007
#[test]
fn rain_is_plausible_per_reading_an_amount_to_300_mm_and_a_counter_from_zero() {
    let amount = Some(RainKind::Amount);
    let flood = judge(Input::Rain, amount, entity(301.0, "mm", 1), None, now());
    assert_eq!(flood, Judged::Refused(Problem::Implausible(301.0)));
    let rain = judge(
        Input::Rain,
        amount,
        entity(0.5, "in", 1),
        previous(0.2, 6),
        now(),
    );
    assert!(
        (stored(rain).1 - 12.7).abs() < 1e-9,
        "an amount is stored as is"
    );

    let counter = Some(RainKind::Counter);
    let below = judge(
        Input::Rain,
        counter,
        entity(-1.0, "mm", 1),
        previous(5.0, 6),
        now(),
    );
    assert_eq!(below, Judged::Refused(Problem::Implausible(-1.0)));
    let high = judge(Input::Rain, counter, entity(900.0, "mm", 1), None, now());
    assert!(
        matches!(high, Judged::Starting { .. }),
        "a counter's total has no ceiling"
    );
}

// @spec WX-SRC-HA-006
#[test]
fn a_counter_gives_its_rise_since_the_previous_reading_and_a_reset_counts_as_zero() {
    let counter = Some(RainKind::Counter);
    let rose = judge(
        Input::Rain,
        counter,
        entity(1.1, "in", 1),
        previous(25.4, 6),
        now(),
    );
    let (_, amount, keep, jumped) = stored(rose);
    assert!((amount - 2.54).abs() < 1e-9, "the rise, in millimetres");
    assert!(
        (keep.value - 27.94).abs() < 1e-9,
        "the total is kept, not the amount"
    );
    assert!(!jumped);

    let dry = judge(
        Input::Rain,
        counter,
        entity(27.94, "mm", 1),
        previous(27.94, 6),
        now(),
    );
    assert_eq!(stored(dry).1, 0.0);

    let reset = judge(
        Input::Rain,
        counter,
        entity(0.3, "mm", 1),
        previous(27.94, 6),
        now(),
    );
    let (_, amount, keep, jumped) = stored(reset);
    assert_eq!(amount, 0.0);
    assert_eq!(keep.value, 0.3, "the new total is the next starting point");
    assert!(!jumped);
}

// @spec WX-SRC-HA-009
#[test]
fn a_counters_first_reading_is_its_starting_point() {
    let first = judge(
        Input::Rain,
        Some(RainKind::Counter),
        entity(12.0, "mm", 2),
        None,
        now(),
    );
    assert_eq!(
        first,
        Judged::Starting {
            keep: Previous {
                value: 12.0,
                at: now() - Duration::minutes(2)
            }
        }
    );
}

// @spec WX-SRC-HA-010
#[test]
fn a_counter_rising_more_than_300_mm_counts_as_zero_and_is_marked() {
    let counter = Some(RainKind::Counter);
    let jump = judge(
        Input::Rain,
        counter,
        entity(410.0, "mm", 1),
        previous(100.0, 6),
        now(),
    );
    let (_, amount, keep, jumped) = stored(jump);
    assert_eq!(amount, 0.0);
    assert_eq!(keep.value, 410.0);
    assert!(jumped);
    let just = judge(
        Input::Rain,
        counter,
        entity(400.0, "mm", 1),
        previous(100.0, 6),
        now(),
    );
    assert_eq!(stored(just).1, 300.0, "300 mm is still rain");
}

// @spec WX-SRC-HA-014
#[test]
fn a_reported_time_more_than_five_minutes_ahead_is_no_reading() {
    let ahead = judge(Input::AirTemp, None, entity(60.0, "°F", -6), None, now());
    assert_eq!(
        ahead,
        Judged::Refused(Problem::Ahead(now() + Duration::minutes(6)))
    );
    let skew = judge(Input::AirTemp, None, entity(60.0, "°F", -5), None, now());
    assert_eq!(
        stored(skew).1,
        60.0,
        "five minutes of clock skew is allowed"
    );
}

// @spec SYNC-SCHED-015
#[test]
fn each_problem_has_a_sentence_naming_it() {
    let unit = Problem::Unit(Some("K".into())).sentence(Input::AirTemp, "sensor.probe");
    assert!(unit.contains("K") && unit.contains("°F or °C"), "{unit}");
    let odd = Problem::Implausible(200.0).sentence(Input::AirTemp, "sensor.probe");
    assert!(odd.contains("200"), "{odd}");
    for problem in [
        Problem::NotANumber,
        Problem::NotFound,
        Problem::Unit(None),
        Problem::Ahead(now()),
    ] {
        let said = problem.sentence(Input::AirTemp, "sensor.probe");
        assert!(said.ends_with('.') && said.len() > 20, "{said}");
    }
}

// @spec WX-SRC-HA-003, WX-SRC-POST-009
#[test]
fn a_reported_time_is_kept_to_the_microsecond_the_record_stores() {
    let stored = Previous {
        value: 60.0,
        at: now() - Duration::minutes(2) + Duration::microseconds(903_139),
    };
    let finer = Lookup::Reading(Entity {
        entity_id: "sensor.probe".into(),
        friendly_name: "Probe".into(),
        value: 61.0,
        unit: Some("°F".into()),
        device_class: None,
        state_class: None,
        reported_at: stored.at + Duration::nanoseconds(783),
    });
    assert_eq!(
        judge(Input::AirTemp, None, finer.clone(), Some(stored), now()),
        Judged::Unchanged,
        "nanoseconds the database cannot hold do not make a reading newer"
    );
    let (at, ..) = stored_parts(judge(Input::AirTemp, None, finer, None, now()));
    assert_eq!(
        at, stored.at,
        "the time is stored as the database will hold it"
    );
}

fn stored_parts(judged: Judged) -> (DateTime<Utc>, f64) {
    match judged {
        Judged::Store { at, value, .. } => (at, value),
        other => panic!("expected a reading to store, got {other:?}"),
    }
}
