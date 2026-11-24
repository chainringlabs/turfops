//! Reading a posted body and deciding what to log.

use super::*;
use chrono::{Duration, TimeZone};
use serde_json::json;

fn received() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 24, 15, 0, 0).unwrap()
}

fn one(entry: serde_json::Value) -> Result<PostedReading, BadReading> {
    let body = json!({ "readings": [entry] });
    readings(&body, received()).unwrap().remove(0)
}

fn problem(entry: serde_json::Value) -> String {
    one(entry)
        .expect_err("expected the reading to be refused")
        .problem
}

// @spec WX-SRC-POST-001, WX-SRC-POST-005
#[test]
fn a_reading_carries_its_sensor_value_unit_and_time_or_the_time_received() {
    let timed = one(json!({
        "sensor": "  Garden probe ", "value": 61.5, "unit": "°F",
        "observed_at": "2026-11-24T09:58:00-05:00"
    }))
    .unwrap();
    assert_eq!(
        timed,
        PostedReading {
            index: 0,
            sensor: "Garden probe".into(),
            value: 61.5,
            unit: "°F".into(),
            observed_at: received() - Duration::minutes(2),
        }
    );
    let untimed = one(json!({"sensor": "Garden probe", "value": 7, "unit": "mm"})).unwrap();
    assert_eq!(untimed.observed_at, received());
    assert_eq!(untimed.value, 7.0, "an integer is a number");
}

// @spec WX-SRC-POST-004
#[test]
fn a_reading_with_a_field_it_cannot_use_is_refused_by_itself() {
    assert!(problem(json!({"value": 1, "unit": "mm"})).contains("sensor"));
    assert!(problem(json!({"sensor": " ", "value": 1, "unit": "mm"})).contains("sensor"));
    let text = problem(json!({"sensor": "Gauge", "value": "1.2", "unit": "mm"}));
    assert!(text.contains("number"), "{text}");
    let unitless = problem(json!({"sensor": "Gauge", "value": 1.2}));
    assert!(unitless.contains("unit"), "{unitless}");
    let local = problem(json!({
        "sensor": "Gauge", "value": 1.2, "unit": "mm", "observed_at": "2026-11-24T15:00:00"
    }));
    assert!(local.contains("UTC offset"), "{local}");
    let refused = one(json!({"sensor": "Gauge", "value": "x", "unit": "mm"})).unwrap_err();
    assert_eq!(
        refused.sensor.as_deref(),
        Some("Gauge"),
        "the problem names the sensor"
    );
}

// @spec WX-SRC-POST-004
#[test]
fn an_empty_unit_is_a_unit_and_is_left_for_the_sensors_input_to_judge() {
    let fraction = one(json!({"sensor": "Probe", "value": 0.31, "unit": ""})).unwrap();
    assert_eq!(fraction.unit, "");
}

// @spec WX-SRC-POST-015
#[test]
fn a_body_that_is_not_a_list_of_readings_is_refused_whole() {
    for body in [
        json!([{"sensor": "Gauge", "value": 1, "unit": "mm"}]),
        json!({"reading": []}),
        json!({"readings": []}),
        json!({"readings": {"sensor": "Gauge"}}),
        json!("readings"),
    ] {
        assert!(readings(&body, received()).is_err(), "{body}");
    }
    let mixed = json!({"readings": [{"sensor": "A", "value": 1, "unit": "mm"}, 42]});
    let taken = readings(&mixed, received()).unwrap();
    assert!(taken[0].is_ok());
    assert_eq!(
        taken[1].as_ref().unwrap_err().index,
        1,
        "a non-object entry is one bad reading"
    );
}

// @spec WX-SRC-POST-008
#[test]
fn readings_are_taken_oldest_first_and_ties_keep_their_order() {
    let at = |index: usize, minutes: i64| PostedReading {
        index,
        sensor: "Gauge".into(),
        value: index as f64,
        unit: "mm".into(),
        observed_at: received() - Duration::minutes(minutes),
    };
    let mut batch = vec![at(0, 1), at(1, 10), at(2, 5), at(3, 5)];
    in_time_order(&mut batch);
    let order: Vec<_> = batch.iter().map(|r| r.index).collect();
    assert_eq!(order, vec![1, 2, 3, 0]);
}

// @spec WX-SRC-POST-007
#[test]
fn names_match_without_regard_to_case_or_surrounding_space() {
    assert!(same_name(" Garden Probe", "garden probe "));
    assert!(!same_name("Garden probe", "Garden  probe"));
    assert!(!same_name("Garden probe", "Garden probe 2"));
}

// @spec WX-SRC-POST-012
#[test]
fn a_sensors_first_stored_reading_and_first_rejection_since_it_last_stored_are_logged() {
    let mut log = PostedLog::default();
    assert!(log.rejected("Gauge"), "the first rejection is logged");
    assert!(!log.rejected("gauge "), "and not the next, by any spelling");
    assert!(log.stored("Gauge"), "the first stored reading since start");
    assert!(!log.stored("Gauge"));
    assert!(
        log.rejected("Gauge"),
        "a rejection after a stored reading is logged again"
    );
    assert!(!log.rejected("Gauge"));
    assert!(log.stored("Probe"), "each sensor on its own");
}

// @spec WX-SRC-POST-005, WX-SRC-POST-009
#[test]
fn a_posted_time_is_kept_to_the_microsecond_the_record_stores() {
    let fine = one(json!({
        "sensor": "Probe", "value": 1, "unit": "mm",
        "observed_at": "2026-11-24T15:00:00.903139783Z"
    }))
    .unwrap();
    assert_eq!(
        fine.observed_at.to_rfc3339(),
        "2026-11-24T15:00:00.903139+00:00"
    );
    let body = json!({"readings": [{"sensor": "Probe", "value": 1, "unit": "mm"}]});
    let arrived = received() + Duration::nanoseconds(903_139_783);
    let untimed = readings(&body, arrived).unwrap().remove(0).unwrap();
    assert_eq!(
        untimed.observed_at,
        received() + Duration::microseconds(903_139)
    );
}
