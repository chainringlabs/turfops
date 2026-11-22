//! The stored basis plan against PostgreSQL.

use super::{catalog, plan, rows, sources};
use crate::logic::weather_record::plan::{build, Lawn, MappedSensor, Pin};
use crate::models::weather_record::*;
use chrono::{TimeZone, Utc};
use sqlx::PgPool;

const LAWN: Lawn = Lawn {
    latitude: 39.86,
    longitude: -75.79,
    elevation_ft: Some(400.0),
};

fn station(network: Network, id: &str, miles_north: f64) -> Station {
    Station {
        network,
        station_id: id.into(),
        name: format!("Station {id}"),
        state: "PA".into(),
        latitude: LAWN.latitude + miles_north / 69.09,
        longitude: LAWN.longitude,
        elevation_ft: Some(400.0),
        inputs: vec![Input::SoilTemp5, Input::SoilTemp10],
        first_year: Some(2006),
        last_year: None,
        locator: None,
        utc_offset_hours: None,
    }
}

async fn stocked(pool: &PgPool) -> Vec<(i64, Station)> {
    let uscrn = [station(Network::Uscrn, "a", 20.0)];
    let scan = [
        station(Network::Scan, "b", 30.0),
        station(Network::Scan, "c", 90.0),
    ];
    catalog::replace_network(pool, Network::Uscrn, &uscrn)
        .await
        .unwrap();
    catalog::replace_network(pool, Network::Scan, &scan)
        .await
        .unwrap();
    catalog::stations(pool).await.unwrap()
}

fn keys(placed: &[plan::Placed], input: Input) -> Vec<(i32, &str, bool)> {
    placed
        .iter()
        .filter(|p| p.input == input)
        .map(|p| (p.rank, p.source.key.as_str(), p.pinned))
        .collect()
}

// @spec WX-REC-PLAN-002, WX-REC-PLAN-003, WX-REC-PLAN-009
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_plan_is_stored_with_its_sources_ranks_and_distances(pool: PgPool) {
    let stations = stocked(&pool).await;
    let sensors = [MappedSensor {
        input: Input::SoilTemp10,
        key: "sensor:1".into(),
        name: "Probe".into(),
    }];
    plan::replace(&pool, &build(Some(&LAWN), &stations, &sensors, &[]))
        .await
        .unwrap();
    let placed = plan::read(&pool).await.unwrap();
    assert_eq!(
        keys(&placed, Input::SoilTemp10),
        vec![
            (1, "sensor:1", false),
            (2, "station:Uscrn:a", false),
            (3, "station:Scan:b", false),
            (4, "estimate:39.8600,-75.7900", false),
        ]
    );
    let uscrn = &placed
        .iter()
        .find(|p| p.source.key == "station:Uscrn:a")
        .unwrap()
        .source;
    assert!((uscrn.distance_miles.unwrap() - 20.0).abs() < 0.05);
    assert_eq!(uscrn.role, SourceRole::Basis);
    // One source however many inputs it is ranked for.
    let all = sources::all(&pool).await.unwrap();
    assert_eq!(all.iter().filter(|s| s.key == "station:Uscrn:a").count(), 1);
}

// @spec WX-REC-PLAN-002, WX-REC-PLAN-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_rebuilt_plan_replaces_the_old_one_and_a_kept_source_keeps_its_rows(pool: PgPool) {
    let stations = stocked(&pool).await;
    plan::replace(&pool, &build(Some(&LAWN), &stations, &[], &[]))
        .await
        .unwrap();
    let before = plan::read(&pool).await.unwrap();
    let kept = before
        .iter()
        .find(|p| p.source.key == "station:Uscrn:a")
        .unwrap();
    let hour = HourlyRow {
        observed_at: Utc.with_ymd_and_hms(2026, 11, 20, 5, 0, 0).unwrap(),
        air_temp_f: Some(41.0),
        ..Default::default()
    };
    let mut conn = pool.acquire().await.unwrap();
    rows::write_hourly(&mut conn, kept.source.id, std::slice::from_ref(&hour))
        .await
        .unwrap();
    drop(conn);

    let far = stations
        .iter()
        .find(|(_, s)| s.station_id == "c")
        .unwrap()
        .0;
    let pins = [Pin {
        input: Input::SoilTemp5,
        station_ref: far,
    }];
    plan::replace(&pool, &build(Some(&LAWN), &stations, &[], &pins))
        .await
        .unwrap();
    let after = plan::read(&pool).await.unwrap();
    assert_eq!(
        keys(&after, Input::SoilTemp5),
        vec![
            (1, "station:Scan:c", true),
            (2, "estimate:39.8600,-75.7900", false)
        ]
    );
    let still = after
        .iter()
        .find(|p| p.source.key == "station:Uscrn:a")
        .unwrap();
    assert_eq!(still.source.id, kept.source.id);
    let from = hour.observed_at - chrono::Duration::hours(1);
    let to = hour.observed_at + chrono::Duration::hours(1);
    let held = rows::hourly(&pool, kept.source.id, from, to).await.unwrap();
    assert_eq!(held, vec![hour]);
}

// @spec WX-REC-PLAN-011
#[sqlx::test(migrations = "src/db/migrations")]
async fn losing_the_location_empties_the_plan(pool: PgPool) {
    let stations = stocked(&pool).await;
    plan::replace(&pool, &build(Some(&LAWN), &stations, &[], &[]))
        .await
        .unwrap();
    plan::replace(&pool, &build(None, &stations, &[], &[]))
        .await
        .unwrap();
    assert!(plan::read(&pool).await.unwrap().is_empty());
}
