//! `SENSOR_TOKEN` as configuration.

use super::*;
use crate::config::test_config;
use crate::error::TurfOpsError;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

// @spec DEPLOY-CONFIG-003, WX-SRC-POST-002
#[test]
fn a_blank_sensor_token_is_not_set() {
    for blank in ["", "  ", "\t\n"] {
        assert_eq!(parse_token(Some(blank.into())).unwrap(), None, "{blank:?}");
    }
    assert_eq!(parse_token(None).unwrap(), None);
    assert!(test_config(&[("SENSOR_TOKEN", " ")])
        .sensor_readings
        .token
        .is_none());
}

// @spec WX-SRC-POST-001
#[test]
fn a_sensor_token_of_32_characters_or_more_is_taken_trimmed() {
    let padded = format!("  {TOKEN}\n");
    assert_eq!(parse_token(Some(padded)).unwrap().as_deref(), Some(TOKEN));
    let config = test_config(&[("SENSOR_TOKEN", TOKEN)]);
    assert_eq!(config.sensor_readings.token.as_deref(), Some(TOKEN));
}

// @spec DEPLOY-CONFIG-014
#[test]
fn a_short_sensor_token_stops_startup_naming_the_setting() {
    match parse_token(Some(TOKEN[..31].into())) {
        Err(TurfOpsError::Config(message)) => {
            assert_eq!(message, "SENSOR_TOKEN must be at least 32 characters")
        }
        other => panic!("expected a configuration error, got {other:?}"),
    }
}

// @spec DEPLOY-CONFIG-010
#[test]
fn the_sensor_token_never_appears_in_the_configurations_debug_output() {
    let config = test_config(&[("SENSOR_TOKEN", TOKEN)]);
    let printed = format!("{config:?}");
    assert!(!printed.contains(TOKEN), "{printed}");
    assert!(printed.contains("SensorReadingsConfig"));
}
