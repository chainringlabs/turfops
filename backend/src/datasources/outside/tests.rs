//! The shared request layer against a local stub.

use super::*;
use crate::test_support::{stub_http, stub_http_seeing};

// @spec WX-SRC-REQ-001
#[tokio::test]
async fn a_request_names_turfops_and_its_version_and_nothing_else_about_the_owner() {
    let (base, request) = stub_http_seeing(200, "ok", "").await;
    Outside::unpaced()
        .get(&format!("{base}/file.txt"), None)
        .await
        .unwrap();
    let request = request.await.unwrap().to_lowercase();
    let agent = format!("user-agent: turfops/{}", env!("CARGO_PKG_VERSION"));
    assert!(request.contains(&agent), "{request}");
    for forbidden in ["from:", "cookie:", "authorization:", "referer:", "@"] {
        assert!(!request.contains(forbidden), "{forbidden} in {request}");
    }
}

// @spec WX-SRC-REQ-004
#[tokio::test]
async fn a_held_copy_is_asked_for_conditionally_and_not_modified_is_a_success() {
    let (base, request) = stub_http_seeing(304, "", "").await;
    let held = DateTime::parse_from_rfc3339("2026-11-20T01:22:53Z")
        .unwrap()
        .with_timezone(&Utc);
    let got = Outside::unpaced()
        .get(&format!("{base}/file.txt"), Some(held))
        .await;
    assert_eq!(got, Ok(Fetched::NotModified));
    let request = request.await.unwrap().to_lowercase();
    assert!(
        request.contains("if-modified-since: fri, 20 nov 2026 01:22:53 gmt"),
        "{request}"
    );
}

// @spec WX-SRC-REQ-004
#[tokio::test]
async fn a_body_comes_back_with_when_the_source_last_changed_it() {
    let (base, _) = stub_http_seeing(
        200,
        "rows",
        "Last-Modified: Fri, 20 Nov 2026 01:22:53 GMT\r\n",
    )
    .await;
    let got = Outside::unpaced()
        .get(&format!("{base}/file.txt"), None)
        .await
        .unwrap();
    let Fetched::Body {
        text,
        last_modified,
    } = got
    else {
        panic!("expected a body");
    };
    assert_eq!(text, "rows");
    assert_eq!(
        last_modified.unwrap().to_rfc3339(),
        "2026-11-20T01:22:53+00:00"
    );
}

// @spec WX-SRC-USCRN-006
#[tokio::test]
async fn a_file_the_source_does_not_have_is_missing_not_a_failure() {
    let base = stub_http(404, "not found").await;
    let got = Outside::unpaced()
        .get(&format!("{base}/file.txt"), None)
        .await;
    assert_eq!(got, Ok(Fetched::Missing));
}

// @spec WX-SRC-REQ-005
#[tokio::test]
async fn a_failure_is_returned_after_one_attempt() {
    let base = stub_http(503, "busy").await;
    let outside = Outside::unpaced();
    let failed = outside
        .get(&format!("{base}/file.txt"), None)
        .await
        .unwrap_err();
    assert!(failed.contains("503"), "{failed}");
    // The one-shot stub answered once; a retry would have met a closed port and said so.
    assert!(!failed.to_lowercase().contains("connect"), "{failed}");
    let refused = outside
        .post_json(
            &format!("{base}/x"),
            &serde_json::json!({}),
            REQUEST_TIMEOUT,
        )
        .await;
    assert!(
        refused.is_err(),
        "the stub is gone: nothing answers a second request"
    );
}

// @spec WX-SRC-REQ-003
#[test]
fn requests_to_a_host_are_a_second_apart_and_two_for_rcc_acis() {
    assert_eq!(pause_for("www.ncei.noaa.gov"), Duration::from_secs(1));
    assert_eq!(pause_for("grid2.rcc-acis.org"), Duration::from_secs(2));
    assert_eq!(pause_for("rcc-acis.org"), Duration::from_secs(2));
    assert_eq!(pause_for("notrcc-acis.org"), Duration::from_secs(1));
}

// @spec WX-SRC-REQ-003
#[tokio::test]
async fn a_second_request_to_the_same_host_waits_its_turn() {
    let first = stub_http(200, "a").await;
    let second = stub_http(200, "b").await;
    let outside = Outside::new();
    let started = std::time::Instant::now();
    outside.get(&format!("{first}/a"), None).await.unwrap();
    outside.get(&format!("{second}/b"), None).await.unwrap();
    assert!(
        started.elapsed() >= Duration::from_millis(950),
        "{:?}",
        started.elapsed()
    );
}

// @spec WX-SRC-REQ-002
#[test]
fn a_request_gives_up_after_30_seconds_and_a_long_grid_request_after_120() {
    assert_eq!(
        (CONNECT_TIMEOUT, REQUEST_TIMEOUT),
        (Duration::from_secs(5), Duration::from_secs(30))
    );
    assert_eq!(LONG_REQUEST_TIMEOUT, Duration::from_secs(120));
}
