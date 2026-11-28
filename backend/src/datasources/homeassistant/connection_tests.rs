//! The Home Assistant connection against a local stub.

use super::*;
use crate::test_support::{stub_http, stub_http_seeing};

const STATES: &str = r#"[
{"entity_id":"sensor.patio_temperature","state":"61.5",
 "attributes":{"friendly_name":"Patio temperature","unit_of_measurement":"°F",
               "device_class":"temperature","state_class":"measurement"},
 "last_changed":"2026-11-23T14:00:00+00:00","last_updated":"2026-11-23T14:00:00+00:00",
 "last_reported":"2026-11-23T14:20:00+00:00"},
{"entity_id":"sensor.old_probe","state":"0.23","attributes":{},
 "last_updated":"2026-11-23T13:00:00+00:00"},
{"entity_id":"sensor.offline","state":"unavailable","attributes":{"device_class":"humidity"},
 "last_updated":"2026-11-23T13:00:00+00:00"},
{"entity_id":"sensor.sun_next_rising","state":"2026-11-24T11:55:00+00:00","attributes":{},
 "last_updated":"2026-11-23T13:00:00+00:00"},
{"entity_id":"light.kitchen","state":"1","attributes":{},
 "last_updated":"2026-11-23T13:00:00+00:00"}
]"#;

const TOKEN: &str = "ha-long-lived-token-0123456789";

// @spec WX-SRC-HA-004, WX-SRC-HA-001, WX-SRC-HA-002
#[tokio::test]
async fn the_listing_is_the_sensor_entities_whose_state_is_a_number() {
    let (base, request) = stub_http_seeing(200, STATES, "").await;
    let found = HaConnection::new(&base, TOKEN)
        .unwrap()
        .entities()
        .await
        .unwrap();
    let ids: Vec<_> = found.iter().map(|e| e.entity_id.as_str()).collect();
    assert_eq!(ids, vec!["sensor.patio_temperature", "sensor.old_probe"]);
    let patio = &found[0];
    assert_eq!(
        (patio.friendly_name.as_str(), patio.value),
        ("Patio temperature", 61.5)
    );
    assert_eq!(patio.unit.as_deref(), Some("°F"));
    assert_eq!(patio.device_class.as_deref(), Some("temperature"));
    assert_eq!(
        patio.reported_at.to_rfc3339(),
        "2026-11-23T14:20:00+00:00",
        "last_reported"
    );
    let old = &found[1];
    assert_eq!(old.friendly_name, "sensor.old_probe", "no friendly name");
    assert_eq!(
        old.reported_at.to_rfc3339(),
        "2026-11-23T13:00:00+00:00",
        "last_updated"
    );

    let request = request.await.unwrap().to_lowercase();
    assert!(request.starts_with("get /api/states "), "{request}");
    assert!(request.contains(&format!("authorization: bearer {TOKEN}")));
}

// @spec WX-SRC-HA-005, SETUP-FAIL-002
#[tokio::test]
async fn a_probe_says_whether_nothing_answered_or_the_token_was_refused() {
    let ok = stub_http(
        200,
        r#"{"location_name":"Home","version":"2026.11.2","unit_system":{}}"#,
    )
    .await;
    let answered = HaConnection::new(&ok, TOKEN)
        .unwrap()
        .probe()
        .await
        .unwrap();
    assert_eq!(
        (answered.location_name.as_str(), answered.version.as_str()),
        ("Home", "2026.11.2")
    );
    let refused = stub_http(401, "401: Unauthorized").await;
    assert_eq!(
        HaConnection::new(&refused, TOKEN).unwrap().probe().await,
        Err(Unusable::Refused)
    );
    let other = stub_http(404, "not found").await;
    assert_eq!(
        HaConnection::new(&other, TOKEN).unwrap().probe().await,
        Err(Unusable::NoAnswer)
    );
    let nothing = HaConnection::new("http://127.0.0.1:1", TOKEN).unwrap();
    assert_eq!(nothing.probe().await, Err(Unusable::NoAnswer));
    assert_eq!(
        Unusable::Refused.sentence(),
        "Home Assistant refused the token."
    );
    assert_eq!(
        Unusable::NoAnswer.sentence(),
        "Home Assistant did not answer at that address."
    );
}

// @spec SETUP-HA-003
#[test]
fn an_address_is_an_http_or_https_url() {
    for bad in [
        "homeassistant.local:8123",
        "ftp://ha.local",
        "file:///etc/passwd",
        "",
        "http://",
    ] {
        assert_eq!(
            HaConnection::new(bad, TOKEN).err(),
            Some(Unusable::BadAddress),
            "{bad}"
        );
    }
    assert!(HaConnection::new(" https://ha.example/ ", TOKEN).is_ok());
}

// @spec WX-SRC-ERR-002, SETUP-CRED-001
#[tokio::test]
async fn no_failure_carries_the_token() {
    let failures = [Unusable::BadAddress, Unusable::NoAnswer, Unusable::Refused];
    for failure in failures {
        let text = format!("{failure:?} {}", failure.sentence());
        assert!(!text.contains(TOKEN));
    }
}

// @spec WX-SRC-HA-001, WX-SRC-HA-002
#[tokio::test]
async fn one_entity_is_its_reading_or_why_there_is_none() {
    let reading = r#"{"entity_id":"sensor.patio_temperature","state":"16.4",
        "attributes":{"unit_of_measurement":"°C"},"last_reported":"2026-11-23T14:20:00+00:00"}"#;
    let (base, request) = stub_http_seeing(200, reading, "").await;
    let connection = HaConnection::new(&base, TOKEN).unwrap();
    let Lookup::Reading(found) = connection.entity("sensor.patio_temperature").await.unwrap()
    else {
        panic!("a reading");
    };
    assert_eq!((found.value, found.unit.as_deref()), (16.4, Some("°C")));
    assert!(request
        .await
        .unwrap()
        .starts_with("GET /api/states/sensor.patio_temperature "));

    let off = r#"{"entity_id":"sensor.x","state":"unavailable","attributes":{},
        "last_updated":"2026-11-23T14:20:00+00:00"}"#;
    let base = stub_http(200, off).await;
    let connection = HaConnection::new(&base, TOKEN).unwrap();
    assert_eq!(connection.entity("sensor.x").await, Ok(Lookup::NotANumber));
    let base = stub_http(404, r#"{"message":"Entity not found."}"#).await;
    let connection = HaConnection::new(&base, TOKEN).unwrap();
    assert_eq!(connection.entity("sensor.gone").await, Ok(Lookup::Missing));
    // An id that is not an id is never sent.
    let nowhere = HaConnection::new("http://127.0.0.1:1", TOKEN).unwrap();
    assert_eq!(
        nowhere.entity("sensor.x/../../config").await,
        Ok(Lookup::Missing)
    );
}

/// What is logged at warn or above while `run` is awaited (on this test's thread).
async fn warnings_while<T>(run: impl std::future::Future<Output = T>) -> (T, String) {
    use std::sync::{Arc, Mutex};
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let sink = buf.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_ansi(false)
        .with_writer(move || Sink(sink.clone()))
        .finish();
    crate::test_support::keep_tracing_interest_shared();
    let guard = tracing::subscriber::set_default(subscriber);
    let out = run.await;
    drop(guard);
    let text = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    (out, text)
}

// @spec WX-SRC-ERR-001, WX-SRC-ERR-002
#[tokio::test]
async fn why_home_assistant_did_not_answer_is_logged_without_its_address_or_token() {
    for (status, body, why) in [
        (500, "boom", "500"),
        (200, "not json", "decod"),
        (401, "401: Unauthorized", "401"),
    ] {
        let base = stub_http(status, body).await;
        // An address may carry a user name and password; neither is logged.
        let address = base.replace("http://", "http://owner:hunter2@");
        let (got, log) =
            warnings_while(async { HaConnection::new(&address, TOKEN).unwrap().probe().await })
                .await;
        assert!(got.is_err(), "{status}");
        assert!(log.contains("/api/config"), "{log}");
        assert!(log.to_lowercase().contains(why), "{why} in {log}");
        for secret in ["hunter2", "owner", TOKEN, "127.0.0.1"] {
            assert!(!log.contains(secret), "{secret} in {log}");
        }
    }
    let (_, log) = warnings_while(async {
        HaConnection::new("http://owner:hunter2@127.0.0.1:1", TOKEN)
            .unwrap()
            .entities()
            .await
    })
    .await;
    assert!(log.contains("/api/states"), "{log}");
    assert!(
        !log.contains("hunter2") && !log.contains("127.0.0.1"),
        "{log}"
    );
}
