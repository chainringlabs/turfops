//! Deciding what to fit on, and holding the data for it, against PostgreSQL.

use super::calibration::Decision;
use super::estimator::SoilEstimator;
use super::scores::Calibration;
use crate::datasources::cadence::Product;
use crate::db::weather_queries::{catalog, days, fetch_state, rows, sources};
use crate::logic::weather_record::due::Purpose;
use crate::logic::weather_record::lab::*;
use crate::models::weather_record::*;
use chrono::{Duration, NaiveDate};
use chrono_tz::America::New_York;
use sqlx::PgPool;

fn train_from() -> NaiveDate {
    NaiveDate::from_ymd_opt(2012, 1, 1).unwrap()
}

/// The lab's station "a" at 20 miles, and six more USCRN stations at 30 to 80.
pub(super) async fn neighbours(lab: &Lab) {
    let mut stations = Vec::new();
    for (id, miles) in [
        ("a", 20.0),
        ("b", 30.0),
        ("c", 40.0),
        ("d", 50.0),
        ("e", 60.0),
        ("f", 70.0),
        ("g", 80.0),
    ] {
        stations.push(Station {
            network: Network::Uscrn,
            station_id: id.into(),
            name: if id == "a" {
                "Avondale 2 N".into()
            } else {
                format!("Station {id}")
            },
            state: "PA".into(),
            latitude: 39.86 + miles / 69.09,
            longitude: -75.79,
            elevation_ft: Some(400.0),
            inputs: vec![Input::SoilTemp5, Input::SoilTemp10],
            first_year: Some(2006),
            last_year: None,
            locator: None,
            utc_offset_hours: None,
        });
    }
    catalog::replace_network(&lab.pool, Network::Uscrn, &stations)
        .await
        .unwrap();
}

async fn calibration_keys(lab: &Lab) -> Vec<String> {
    let mut keys: Vec<String> = sources::all(&lab.pool)
        .await
        .unwrap()
        .into_iter()
        .filter(|s| s.role == SourceRole::Calibration)
        .map(|s| s.key)
        .collect();
    keys.sort();
    keys
}

/// Marks a calibration station's history as held, with `soil_days` days of soil in it.
async fn hold(lab: &Lab, id: &str, soil_days: i64) {
    let source = lab.source(&format!("calibration:Uscrn:{id}")).await;
    let rows: Vec<DailyRow> = (0..soil_days)
        .map(|n| DailyRow {
            day: train_from() + Duration::days(n),
            soil_temp_5_f: Some(60.0),
            soil_temp_10_f: Some(58.0),
            ..Default::default()
        })
        .collect();
    let mut conn = lab.pool.acquire().await.unwrap();
    rows::write_daily(&mut conn, &source, &rows).await.unwrap();
    sqlx::query(
        "UPDATE weather_fetch_state SET held_from = wanted_from, held_through = $2 \
         WHERE source_id = $1",
    )
    .bind(source.id)
    .bind(d(19))
    .execute(&lab.pool)
    .await
    .unwrap();
}

pub(super) fn chosen(decision: &Decision, lab_catalog: &[(i64, Station)]) -> Vec<String> {
    let stations = match decision {
        Decision::Fit(_, stations) | Decision::Wait(stations) => stations,
        Decision::NoModel => return Vec::new(),
    };
    stations
        .iter()
        .map(|c| {
            let station = lab_catalog
                .iter()
                .find(|(id, _)| *id == c.station_ref)
                .unwrap();
            station.1.station_id.clone()
        })
        .collect()
}

// @spec WX-EST-FAIL-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_lawn_with_no_location_has_nothing_to_decide(pool: PgPool) {
    let decisions = SoilEstimator::new(pool).prepare(d(20)).await.unwrap();
    assert!(decisions.is_empty());
}

// @spec WX-EST-CAL-001, WX-EST-CAL-003, WX-EST-CAL-007, WX-REC-STORE-008
#[sqlx::test(migrations = "src/db/migrations")]
async fn with_too_little_from_the_basis_station_the_five_nearest_are_held_and_waited_for(
    pool: PgPool,
) {
    let lab = Lab::new(pool).await;
    neighbours(&lab).await;
    let estimator = SoilEstimator::new(lab.pool.clone());
    let decisions = estimator.prepare(d(20)).await.unwrap();
    let catalog = catalog::stations(&lab.pool).await.unwrap();
    assert_eq!(decisions.len(), 2, "5 cm and 10 cm");
    for (_, decision) in &decisions {
        assert!(matches!(decision, Decision::Wait(_)), "{decision:?}");
        assert_eq!(chosen(decision, &catalog), vec!["b", "c", "d", "e", "f"]);
    }
    let keys = calibration_keys(&lab).await;
    assert_eq!(
        keys.len(),
        10,
        "each station and the grid at its point: {keys:?}"
    );
    assert!(keys.contains(&"calibration:Uscrn:b".to_string()));
    assert!(
        keys.iter()
            .filter(|key| key.starts_with("calibration:grid:prism:"))
            .count()
            == 5
    );

    let station = lab.source("calibration:Uscrn:b").await;
    assert!((station.distance_miles.unwrap() - 30.0).abs() < 0.05);
    let states = fetch_state::all(&lab.pool).await.unwrap();
    let state = states.iter().find(|s| s.source_id == station.id).unwrap();
    assert_eq!(
        (state.product.as_str(), state.wanted_from),
        ("calibration_days", train_from())
    );
    assert_eq!(state.held, None);

    // The record now asks for them.
    let due = lab.record.due(noon(20).at.now, d(20), 0).await.unwrap();
    let for_b: Vec<_> = due
        .fetches
        .iter()
        .filter(|f| f.source.id == station.id)
        .collect();
    assert_eq!(for_b[0].product, Product::CalibrationDays);
    assert_eq!(for_b[0].purpose, Purpose::Current);
    assert_eq!(for_b.last().unwrap().span.0, train_from());
    let grids = due
        .fetches
        .iter()
        .filter(|f| f.source.key.starts_with("calibration:grid") && f.purpose == Purpose::Current)
        .count();
    assert_eq!(grids, 5);
}

// @spec WX-EST-CAL-008, WX-EST-FAIL-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_station_that_proves_short_is_replaced_and_is_not_chosen_again(pool: PgPool) {
    let lab = Lab::new(pool).await;
    neighbours(&lab).await;
    let estimator = SoilEstimator::new(lab.pool.clone());
    estimator.prepare(d(20)).await.unwrap();
    let catalog = catalog::stations(&lab.pool).await.unwrap();
    for id in ["b", "c", "d", "e"] {
        hold(&lab, id, 1000).await;
    }
    hold(&lab, "f", 10).await;

    let decisions = estimator.prepare(d(20)).await.unwrap();
    assert!(matches!(decisions[0].1, Decision::Wait(_)));
    assert_eq!(
        chosen(&decisions[0].1, &catalog),
        vec!["b", "c", "d", "e", "g"]
    );
    let keys = calibration_keys(&lab).await;
    assert!(
        keys.contains(&"calibration:Uscrn:g".to_string()),
        "{keys:?}"
    );
    // The short station's own days stay, so it is not chosen again; the grid at its
    // point is let go.
    assert!(keys.contains(&"calibration:Uscrn:f".to_string()));
    assert_eq!(keys.iter().filter(|key| key.contains("grid")).count(), 5);
    assert_eq!(keys.len(), 11);

    hold(&lab, "g", 1200).await;
    let decisions = estimator.prepare(d(20)).await.unwrap();
    for (_, decision) in &decisions {
        assert!(
            matches!(decision, Decision::Fit(Calibration::NearestFive, _)),
            "{decision:?}"
        );
        assert_eq!(chosen(decision, &catalog), vec!["b", "c", "d", "e", "g"]);
    }
}

// @spec WX-EST-CAL-002, WX-EST-MODEL-006
#[sqlx::test(migrations = "src/db/migrations")]
async fn a_basis_station_with_a_thousand_days_is_fitted_on_alone_with_the_grid_at_its_point(
    pool: PgPool,
) {
    let lab = Lab::new(pool).await;
    neighbours(&lab).await;
    // A thousand days of twelve hours each at 10 cm; 5 cm has only a hundred.
    let start = days::start_of_day(New_York, NaiveDate::from_ymd_opt(2020, 1, 1).unwrap());
    let hours: Vec<HourlyRow> = (0..1000i64)
        .flat_map(|day| (0..12i64).map(move |hour| (day, hour)))
        .map(|(day, hour)| {
            let mut row = HourlyRow {
                observed_at: start + Duration::days(day) + Duration::hours(hour),
                ..Default::default()
            };
            row.soil_temp_f[1] = Some(55.0);
            row.soil_temp_f[0] = (day < 100).then_some(57.0);
            row
        })
        .collect();
    lab.write_hours(STATION, &hours).await;

    let decisions = SoilEstimator::new(lab.pool.clone())
        .prepare(d(20))
        .await
        .unwrap();
    let catalog = catalog::stations(&lab.pool).await.unwrap();
    let ten = &decisions
        .iter()
        .find(|(depth, _)| *depth == Input::SoilTemp10)
        .unwrap()
        .1;
    assert!(
        matches!(ten, Decision::Fit(Calibration::StationAlone, _)),
        "{ten:?}"
    );
    assert_eq!(chosen(ten, &catalog), vec!["a"]);
    let five = &decisions
        .iter()
        .find(|(depth, _)| *depth == Input::SoilTemp5)
        .unwrap()
        .1;
    assert_eq!(
        chosen(five, &catalog),
        vec!["b", "c", "d", "e", "f"],
        "{five:?}"
    );

    let keys = calibration_keys(&lab).await;
    assert_eq!(
        keys.len(),
        11,
        "the five for 5 cm, and the grid at the basis station: {keys:?}"
    );
    assert!(
        !keys.contains(&"calibration:Uscrn:a".to_string()),
        "its soil is its hourly rows"
    );
}
