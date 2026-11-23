//! What is wanted, what is due, source status and clearing out, against PostgreSQL.

use super::due::Purpose;
use super::lab::*;
use super::record::WeatherRecord;
use crate::datasources::cadence::Product;
use crate::db::weather_queries::{fetch_state, sources};
use crate::models::weather_record::*;
use chrono::{Duration, NaiveDate};
use sqlx::PgPool;

fn jan1(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 1, 1).unwrap()
}

/// A lawn whose plan has just been built: nothing registered, nothing held.
async fn new_lawn(pool: PgPool) -> Lab {
    let lab = Lab::new(pool).await;
    // The lab's clock is July 2026: its sources came into use on the 1st.
    for statement in [
        "DELETE FROM weather_fetch_state",
        "UPDATE weather_source SET created_at = '2026-07-01T12:00:00Z'",
    ] {
        sqlx::query(statement).execute(&lab.pool).await.unwrap();
    }
    lab
}

async fn state(lab: &Lab, key: &str) -> Option<FetchState> {
    let id = lab.source(key).await.id;
    let all = fetch_state::all(&lab.pool).await.unwrap();
    all.into_iter().find(|s| s.source_id == id)
}

// @spec WX-REC-STORE-008, WX-REC-FILL-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn each_fetched_source_in_the_plan_is_registered_with_what_it_is_wanted_for(pool: PgPool) {
    let lab = new_lawn(pool).await;
    lab.record.register_wanted(d(20)).await.unwrap();
    let station = state(&lab, STATION).await.unwrap();
    assert_eq!(
        (station.product.as_str(), station.wanted_from),
        ("station_hours", jan1(2011))
    );
    assert_eq!(station.held, None, "a new source holds nothing");
    assert_eq!(state(&lab, PRISM).await.unwrap().wanted_from, jan1(2011));
    let analysis = state(&lab, ANALYSIS).await.unwrap();
    assert_eq!(analysis.wanted_from, d(1) - Duration::days(93));
    assert!(
        state(&lab, ESTIMATE).await.is_none(),
        "the estimate is computed, not fetched"
    );

    // Registering again keeps what has been fetched since.
    let id = lab.source(STATION).await.id;
    fetch_state::hourly_fetched(
        &lab.pool,
        id,
        "station_hours",
        &[],
        (d(1), d(20)),
        noon(20).at.now,
    )
    .await
    .unwrap();
    lab.record.register_wanted(d(20)).await.unwrap();
    assert_eq!(
        state(&lab, STATION).await.unwrap().held,
        Some((d(1), d(20)))
    );
}

// @spec WX-REC-FILL-002, WX-REC-FILL-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_new_lawn_is_due_its_present_then_its_history_and_the_missing_catalog(pool: PgPool) {
    let lab = new_lawn(pool).await;
    lab.record.register_wanted(d(20)).await.unwrap();
    let due = lab.record.due(noon(20).at.now, d(20)).await.unwrap();
    assert_eq!(
        due.catalogs,
        vec![Network::Scan],
        "the lab loaded USCRN only"
    );
    let current: Vec<_> = due
        .fetches
        .iter()
        .filter(|f| f.purpose == Purpose::Current)
        .map(|f| (f.product, f.span.1))
        .collect();
    assert_eq!(current.len(), 4);
    for product in [
        Product::StationHours,
        Product::PrismDays,
        Product::NrccDays,
        Product::AnalysisHours,
    ] {
        assert!(current.contains(&(product, d(20))), "{product:?}");
    }
    let backfill: Vec<_> = due
        .fetches
        .iter()
        .filter(|f| f.purpose == Purpose::Backfill)
        .collect();
    assert_eq!(
        backfill[0].span.1.to_string(),
        "2026-06-19",
        "PRISM's year so far"
    );
    assert_eq!(
        backfill.last().unwrap().span,
        (jan1(2011), NaiveDate::from_ymd_opt(2011, 12, 31).unwrap())
    );
}

// @spec WX-REC-FILL-002, WX-REC-PLAN-011
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_no_plan_nothing_is_due(pool: PgPool) {
    let record = WeatherRecord::new(pool);
    let due = record.due(noon(20).at.now, d(20)).await.unwrap();
    assert!(due.catalogs.is_empty() && due.fetches.is_empty());
    // Setup may still ask which station lists are missing.
    assert_eq!(record.catalogs_due(noon(20).at.now).await.unwrap().len(), 2);
}

// @spec WX-REC-FILL-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn status_has_one_entry_per_source_in_the_plan_with_what_it_is_ranked_for(pool: PgPool) {
    let lab = Lab::new(pool).await;
    let id = lab.source(STATION).await.id;
    let failed_at = noon(20).at.now;
    fetch_state::failed(&lab.pool, id, "station_hours", "timed out", failed_at)
        .await
        .unwrap();
    let status = lab.record.status().await.unwrap();
    assert_eq!(status.len(), 5, "station, estimate, two grids, analysis");
    let station = status.iter().find(|s| s.source_id == id).unwrap();
    assert_eq!(station.name, "Avondale 2 N, PA (USCRN)");
    assert_eq!(station.kind, SourceKind::Station);
    assert_eq!(station.inputs, vec![Input::SoilTemp10, Input::SoilTemp5]);
    assert_eq!(station.last_failure, Some(failed_at));
    assert_eq!(
        (station.wanted_from, station.held),
        (Some(jan1(2011)), Some((jan1(2011), d(19))))
    );
    let analysis = status
        .iter()
        .find(|s| s.kind == SourceKind::Analysis)
        .unwrap();
    assert_eq!(
        analysis.inputs,
        vec![Input::AirTemp, Input::Humidity, Input::Rain]
    );
}

fn stray(key: &str, kind: SourceKind, role: SourceRole) -> NewSource {
    NewSource {
        key: key.into(),
        kind,
        name: key.into(),
        station_ref: None,
        latitude: None,
        longitude: None,
        distance_miles: None,
        role,
    }
}

// @spec WX-REC-FILL-007
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_source_the_plan_dropped_goes_once_its_replacements_are_filled(pool: PgPool) {
    let lab = Lab::new(pool).await;
    for (key, kind, role) in [
        (
            "grid:prism:41.0000,-80.0000",
            SourceKind::Grid,
            SourceRole::Basis,
        ),
        ("sensor:9", SourceKind::Sensor, SourceRole::Basis),
        (
            "station:Uscrn:cal",
            SourceKind::Station,
            SourceRole::Calibration,
        ),
    ] {
        sources::ensure(&lab.pool, &stray(key, kind, role))
            .await
            .unwrap();
    }
    // The station's history is not in yet: the old point's rows stay.
    let id = lab.source(STATION).await.id;
    sqlx::query("UPDATE weather_fetch_state SET held_from = $2 WHERE source_id = $1")
        .bind(id)
        .bind(jan1(2020))
        .execute(&lab.pool)
        .await
        .unwrap();
    assert_eq!(lab.record.drop_replaced().await.unwrap(), 0);

    sqlx::query("UPDATE weather_fetch_state SET held_from = wanted_from WHERE source_id = $1")
        .bind(id)
        .execute(&lab.pool)
        .await
        .unwrap();
    assert_eq!(lab.record.drop_replaced().await.unwrap(), 1);
    let left: Vec<String> = sources::all(&lab.pool)
        .await
        .unwrap()
        .into_iter()
        .map(|s| s.key)
        .collect();
    assert!(!left.iter().any(|key| key.contains("41.0000")), "{left:?}");
    assert!(
        left.contains(&"sensor:9".to_string()) && left.contains(&"station:Uscrn:cal".to_string())
    );
    assert_eq!(left.len(), 7);
}
