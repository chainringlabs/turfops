//! The station catalog and the sources in use, against PostgreSQL.

use super::{catalog, sources};
use crate::models::weather_record::*;

use sqlx::PgPool;

fn station(id: &str, name: &str) -> Station {
    Station {
        network: Network::Uscrn,
        station_id: id.into(),
        name: name.into(),
        state: "PA".into(),
        latitude: 39.86,
        longitude: -75.79,
        elevation_ft: Some(400.0),
        inputs: vec![
            Input::SoilTemp5,
            Input::SoilTemp10,
            Input::AirTemp,
            Input::Humidity,
        ],
        first_year: Some(2006),
        last_year: None,
        locator: Some("PA_Avondale_2_N".into()),
        utc_offset_hours: None,
    }
}

fn new_source(key: &str, kind: SourceKind, role: SourceRole) -> NewSource {
    NewSource {
        key: key.into(),
        kind,
        name: key.into(),
        station_ref: None,
        latitude: Some(39.86),
        longitude: Some(-75.79),
        distance_miles: None,
        role,
    }
}

async fn source(pool: &PgPool, key: &str, kind: SourceKind) -> Source {
    sources::ensure(pool, &new_source(key, kind, SourceRole::Basis))
        .await
        .unwrap()
}

// @spec WX-REC-PLAN-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_catalog_holds_each_station_with_what_it_reports(pool: PgPool) {
    let avondale = station("3761", "Avondale 2 N");
    catalog::replace_network(&pool, Network::Uscrn, std::slice::from_ref(&avondale))
        .await
        .unwrap();
    let held = catalog::stations(&pool).await.unwrap();
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].1, avondale);
}

// @spec WX-REC-PLAN-001, WX-REC-FILL-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_refresh_updates_stations_in_place_and_drops_those_no_longer_listed(pool: PgPool) {
    let uscrn = [
        station("3761", "Avondale 2 N"),
        station("4726", "Ithaca 13 E"),
    ];
    catalog::replace_network(&pool, Network::Uscrn, &uscrn)
        .await
        .unwrap();
    let scan = Station {
        network: Network::Scan,
        ..station("2028", "Mahantango Ck")
    };
    catalog::replace_network(&pool, Network::Scan, &[scan])
        .await
        .unwrap();
    let before = catalog::stations(&pool).await.unwrap();
    let avondale_id = before
        .iter()
        .find(|(_, s)| s.station_id == "3761")
        .unwrap()
        .0;

    let renamed = Station {
        name: "Avondale 2 North".into(),
        ..station("3761", "")
    };
    catalog::replace_network(&pool, Network::Uscrn, &[renamed])
        .await
        .unwrap();
    let after = catalog::stations(&pool).await.unwrap();
    let names: Vec<&str> = after.iter().map(|(_, s)| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Mahantango Ck", "Avondale 2 North"],
        "SCAN is untouched"
    );
    assert_eq!(after[1].0, avondale_id, "a known station keeps its row");
}

// @spec WX-REC-FILL-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_station_a_source_still_uses_survives_leaving_the_networks_list(pool: PgPool) {
    catalog::replace_network(&pool, Network::Uscrn, &[station("3761", "Avondale 2 N")])
        .await
        .unwrap();
    let id = catalog::stations(&pool).await.unwrap()[0].0;
    let in_use = NewSource {
        station_ref: Some(id),
        ..new_source("station:Uscrn:3761", SourceKind::Station, SourceRole::Basis)
    };
    sources::ensure(&pool, &in_use).await.unwrap();
    catalog::replace_network(&pool, Network::Uscrn, &[])
        .await
        .unwrap();
    assert_eq!(catalog::stations(&pool).await.unwrap().len(), 1);
    assert_eq!(sources::all(&pool).await.unwrap().len(), 1);
}

// @spec WX-REC-STORE-001, WX-REC-PLAN-009
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_source_is_known_by_its_key_and_keeps_its_id_when_described_again(pool: PgPool) {
    let first = source(&pool, "station:Uscrn:3761", SourceKind::Station).await;
    let again = sources::ensure(
        &pool,
        &NewSource {
            distance_miles: Some(12.4),
            name: "Avondale 2 N".into(),
            ..new_source("station:Uscrn:3761", SourceKind::Station, SourceRole::Basis)
        },
    )
    .await
    .unwrap();
    assert_eq!(again.id, first.id);
    assert_eq!(
        (again.distance_miles, again.name.as_str()),
        (Some(12.4), "Avondale 2 N")
    );
    assert_eq!(sources::all(&pool).await.unwrap().len(), 1);
}

// @spec WX-REC-PLAN-001, WX-REC-PLAN-002
#[sqlx::test(migrations = "src/db/migrations")]
async fn inputs_learned_from_a_stations_data_survive_a_refresh_that_does_not_list_them(
    pool: PgPool,
) {
    let mut listed = station("3761", "Avondale 2 N");
    listed.inputs = Vec::new();
    catalog::replace_network(&pool, Network::Uscrn, std::slice::from_ref(&listed))
        .await
        .unwrap();
    let id = catalog::stations(&pool).await.unwrap()[0].0;
    let learned = [Input::AirTemp, Input::SoilTemp10];
    assert!(catalog::set_inputs(&pool, id, &learned).await.unwrap());
    assert!(
        !catalog::set_inputs(&pool, id, &learned).await.unwrap(),
        "nothing changed"
    );
    catalog::replace_network(&pool, Network::Uscrn, std::slice::from_ref(&listed))
        .await
        .unwrap();
    assert_eq!(catalog::stations(&pool).await.unwrap()[0].1.inputs, learned);
}
