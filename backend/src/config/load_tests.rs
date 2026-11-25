use super::load::from_vars;
use super::Config;
use crate::error::TurfOpsError;
use std::collections::HashMap;

const DB_SECRET: &str = "s3cret-db-pass";
const HA_SECRET: &str = "ha-long-lived-token";
const OWM_SECRET: &str = "owm-key-123";
const OR_SECRET: &str = "sk-or-v1-abc";
const MCP_SECRET: &str = "0123456789abcdef0123456789abcdef";

fn base() -> HashMap<&'static str, String> {
    HashMap::from([("DATABASE_PASSWORD", DB_SECRET.to_string())])
}

fn load(vars: &HashMap<&'static str, String>) -> crate::error::Result<(Config, Vec<String>)> {
    from_vars(&|key| vars.get(key).cloned())
}

fn with(pairs: &[(&'static str, &str)]) -> HashMap<&'static str, String> {
    let mut vars = base();
    for (k, v) in pairs {
        vars.insert(k, v.to_string());
    }
    vars
}

// @spec DEPLOY-CONFIG-002
#[test]
fn a_missing_or_empty_database_password_stops_startup() {
    for vars in [
        HashMap::new(),
        with(&[("DATABASE_PASSWORD", "")]),
        with(&[("DATABASE_PASSWORD", "  ")]),
    ] {
        match load(&vars) {
            Err(TurfOpsError::Config(msg)) => assert!(msg.contains("DATABASE_PASSWORD"), "{msg}"),
            other => panic!("expected a configuration error, got ok={}", other.is_ok()),
        }
    }
    assert!(load(&base()).is_ok());
}

// @spec DEPLOY-CONFIG-012
#[test]
fn an_unusable_setting_is_named_with_the_default_used() {
    let vars = with(&[("SERVER_HOST", "my-laptop"), ("DB_MAX_CONNECTIONS", "lots")]);
    let (config, warnings) = load(&vars).unwrap();
    assert_eq!(config.server.host, "0.0.0.0");
    assert_eq!(config.database.max_connections, 10);
    for (var, default) in [("SERVER_HOST", "0.0.0.0"), ("DB_MAX_CONNECTIONS", "10")] {
        assert!(
            warnings
                .iter()
                .any(|w| w.contains(var) && w.contains(default)),
            "{var}: {warnings:?}"
        );
    }
    let (_, none) = load(&with(&[("SERVER_HOST", "127.0.0.1")])).unwrap();
    assert!(none.is_empty(), "{none:?}");
}

// @spec DEPLOY-CONFIG-015
#[test]
fn each_retired_setting_that_is_set_is_named_once_as_having_no_effect() {
    let retired = [
        ("DATALAKE_ROOT", "/data"),
        ("WEATHER_SILVER_PATH", "/data/silver.parquet"),
        ("WEATHER_GOLD_PATH", "/data/gold.parquet"),
        ("NOAA_STATION_WBANNO", "3761"),
        ("HA_TEMPERATURE_UNIT", "kelvin"),
    ];
    let (_, warnings) = load(&with(&retired)).unwrap();
    for (name, _) in retired {
        let lines: Vec<&String> = warnings.iter().filter(|w| w.starts_with(name)).collect();
        assert_eq!(lines.len(), 1, "{name}: {warnings:?}");
        assert!(
            lines[0].contains("no longer has any effect"),
            "{}",
            lines[0]
        );
    }
    let (_, none) = load(&with(&[("DATALAKE_ROOT", "  ")])).unwrap();
    assert!(none.is_empty(), "blank is not set: {none:?}");
}

// @spec DEPLOY-CONFIG-013
#[test]
fn feature_flags_accept_true_1_or_yes_in_any_case() {
    let enabled = |value: Option<&str>| {
        let mut vars = with(&[("OPENROUTER_API_KEY", OR_SECRET)]);
        if let Some(v) = value {
            vars.insert("OPENROUTER_ENABLED", v.to_string());
        }
        let (config, _) = load(&vars).unwrap();
        config.openrouter.unwrap().enabled
    };
    for on in ["true", "TRUE", "1", "yes", "Yes"] {
        assert!(enabled(Some(on)), "{on}");
    }
    for off in ["false", "0", "no", "off", ""] {
        assert!(!enabled(Some(off)), "{off:?}");
    }
    assert!(enabled(None), "unset = on");
}

// @spec DEPLOY-LOG-003, DEPLOY-CONFIG-010
#[test]
fn no_credential_appears_in_the_configurations_debug_output() {
    let vars = with(&[
        ("HA_TOKEN", HA_SECRET),
        ("OWM_API_KEY", OWM_SECRET),
        ("OPENROUTER_API_KEY", OR_SECRET),
        ("MCP_TOKEN", MCP_SECRET),
    ]);
    let (config, _) = load(&vars).unwrap();
    let printed = format!("{config:?}");
    for secret in [DB_SECRET, HA_SECRET, OWM_SECRET, OR_SECRET, MCP_SECRET] {
        assert!(!printed.contains(secret), "{secret} leaked");
    }
}

// @spec UI-SRV-009
#[test]
fn an_invalid_cors_origin_stops_startup_without_a_panic() {
    for bad in [
        "lawn.example.com",
        "https://lawn.example.com/app",
        "ftp://lawn.example.com",
        "http://",
    ] {
        match load(&with(&[("CORS_ALLOWED_ORIGIN", bad)])) {
            Err(TurfOpsError::Config(msg)) => assert!(msg.contains("CORS_ALLOWED_ORIGIN"), "{msg}"),
            other => panic!(
                "{bad}: expected a configuration error, got ok={}",
                other.is_ok()
            ),
        }
    }
    for good in [
        "*",
        "http://localhost:5173",
        "https://lawn.example.com",
        "  ",
    ] {
        assert!(
            load(&with(&[("CORS_ALLOWED_ORIGIN", good)])).is_ok(),
            "{good}"
        );
    }
}

// @spec SETUP-CRED-002, DEPLOY-CONFIG-017
#[test]
fn the_forecast_key_is_read_on_its_own() {
    let (config, warnings) = load(&with(&[("OWM_API_KEY", OWM_SECRET)])).unwrap();
    assert_eq!(config.openweathermap.unwrap().api_key, OWM_SECRET);
    assert!(warnings.iter().all(|w| !w.contains("OWM_")), "{warnings:?}");
}

// @spec DEPLOY-CONFIG-017
#[test]
fn the_forecasts_retired_settings_are_each_named_once_as_having_no_effect() {
    let (config, warnings) = load(&with(&[
        ("OWM_API_KEY", OWM_SECRET),
        ("OWM_ENABLED", "false"),
        ("OWM_LATITUDE", "39.85"),
        ("OWM_LONGITUDE", "-75.78"),
    ]))
    .unwrap();
    assert_eq!(
        config.openweathermap.unwrap().api_key,
        OWM_SECRET,
        "OWM_ENABLED=false turns nothing off"
    );
    for name in ["OWM_ENABLED", "OWM_LATITUDE", "OWM_LONGITUDE"] {
        let lines: Vec<_> = warnings.iter().filter(|w| w.contains(name)).collect();
        assert_eq!(lines.len(), 1, "{name}: {warnings:?}");
        assert!(lines[0].contains("no longer has any effect"), "{lines:?}");
        assert!(lines[0].contains("lawn's location"), "{lines:?}");
    }

    // An empty value is not set (Compose passes unset variables as "").
    let (_, quiet) = load(&with(&[
        ("OWM_ENABLED", ""),
        ("OWM_LATITUDE", " "),
        ("OWM_LONGITUDE", ""),
    ]))
    .unwrap();
    assert!(quiet.iter().all(|w| !w.contains("OWM_")), "{quiet:?}");
}
