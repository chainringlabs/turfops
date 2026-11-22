//! The USCRN client against a stubbed network.

use super::*;
use crate::test_support::{stub_http, stub_http_seeing};
use chrono::TimeZone;

const HOUR: &str = "03761 20260118 2200 20260118 1700  2.622  -75.79   39.86    -0.9    -0.5    -0.1    -0.9     0.8      6 0     18 0      0 0 C    -0.9 0    -0.8 0    -1.2 0    88 0   0.357   0.348   0.343   0.350   0.301     1.3     1.5     1.5     2.8     3.8\n";

fn client(base: &str) -> UscrnClient {
    UscrnClient::new(Arc::new(Outside::unpaced())).with_base(base)
}

// @spec WX-SRC-USCRN-002
#[tokio::test]
async fn a_station_years_hours_are_fetched_from_its_file() {
    let (base, request) = stub_http_seeing(
        200,
        HOUR,
        "Last-Modified: Fri, 20 Nov 2026 01:22:53 GMT\r\n",
    )
    .await;
    let got = client(&base)
        .hourly("PA_Avondale_2_N", 2026, None)
        .await
        .unwrap();
    let StationYear::Rows {
        rows,
        last_modified,
    } = got
    else {
        panic!("expected rows");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(
        last_modified,
        Some(Utc.with_ymd_and_hms(2026, 11, 20, 1, 22, 53).unwrap())
    );
    let request = request.await.unwrap();
    assert!(
        request.starts_with("GET /hourly02/2026/CRNH0203-2026-PA_Avondale_2_N.txt "),
        "{request}"
    );
}

// @spec WX-SRC-USCRN-005
#[tokio::test]
async fn a_station_years_days_are_fetched_from_the_daily_file() {
    let (base, request) = stub_http_seeing(200, "", "").await;
    client(&base)
        .daily("AL_Troy_2_W", 2020, None)
        .await
        .unwrap();
    let request = request.await.unwrap();
    assert!(
        request.starts_with("GET /daily01/2020/CRND0103-2020-AL_Troy_2_W.txt "),
        "{request}"
    );
}

// @spec WX-SRC-USCRN-006
#[tokio::test]
async fn a_year_the_station_has_no_file_for_is_an_empty_success() {
    let base = stub_http(404, "Not Found").await;
    let got = client(&base)
        .hourly("PA_Avondale_2_N", 1999, None)
        .await
        .unwrap();
    assert_eq!(
        got,
        StationYear::Rows {
            rows: Vec::new(),
            last_modified: None
        }
    );
}

// @spec WX-SRC-REQ-004
#[tokio::test]
async fn an_unchanged_file_returns_no_rows_and_says_so() {
    let (base, request) = stub_http_seeing(304, "", "").await;
    let held = Utc.with_ymd_and_hms(2026, 11, 20, 1, 22, 53).unwrap();
    let got = client(&base)
        .hourly("PA_Avondale_2_N", 2026, Some(held))
        .await
        .unwrap();
    assert_eq!(got, StationYear::Unchanged);
    assert!(request
        .await
        .unwrap()
        .to_lowercase()
        .contains("if-modified-since:"));
}

// @spec WX-SRC-ERR-001
#[tokio::test]
async fn a_failed_or_unreadable_answer_is_an_outage_of_the_station_network() {
    for (status, body) in [(503, "busy"), (200, "<html>maintenance</html>")] {
        let base = stub_http(status, body).await;
        let err = client(&base)
            .hourly("PA_Avondale_2_N", 2026, None)
            .await
            .unwrap_err();
        let message = err.owner_message();
        assert!(message.contains("station network"), "{message}");
        assert!(
            !message.contains("127.0.0.1") && !message.contains("maintenance"),
            "{message}"
        );
        let TurfOpsError::Unavailable { origin, .. } = err else {
            panic!("expected an outage");
        };
        assert_eq!(origin, Source::SoilStation);
    }
}

// @spec WX-SRC-USCRN-001, WX-SRC-ERR-001
#[tokio::test]
async fn the_catalog_is_read_from_the_station_list_and_an_empty_one_is_an_outage() {
    const LIST: &str = "WBAN\tCOUNTRY\tSTATE\tLOCATION\tVECTOR\tNAME\tLATITUDE\tLONGITUDE\tELEVATION\tSTATUS\tCOMMISSIONING\tCLOSING\tOPERATION\tPAIRING\tNETWORK\tSTATION_ID\n03761\tUS\tPA\tAvondale\t2 N\tStroud Water Research Center\t39.85\t-75.78\t400\tCommissioned\t2006-09-29 00:00:00.0\t\tOperational\t\tUSCRN\t1123\n";
    let (base, request) = stub_http_seeing(200, LIST, "").await;
    let stations = client(&base).catalog().await.unwrap();
    assert_eq!(stations[0].station_id, "03761");
    assert!(request.await.unwrap().starts_with("GET /stations.tsv "));

    let base = stub_http(200, "an error page").await;
    assert!(client(&base).catalog().await.is_err());
}
