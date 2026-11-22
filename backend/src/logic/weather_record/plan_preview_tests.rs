//! Previewing a point before it is saved.

use super::tests::{all, catalog, station, AIR, LAWN, SOIL};
use super::*;

// @spec WX-REC-PLAN-012
#[test]
fn a_preview_gives_the_plan_and_the_stations_within_150_miles_nearest_first() {
    let lawn = Lawn {
        elevation_ft: Some(400.0),
        ..LAWN
    };
    let stations = catalog(vec![
        station(Network::Scan, "mid", 100.0, 1900.0, &SOIL),
        station(Network::Uscrn, "near", 20.0, 350.0, &all(&[&SOIL, &AIR])),
        station(Network::Uscrn, "gone", 151.0, 400.0, &SOIL),
    ]);
    let preview = preview(&lawn, &stations);
    assert_eq!(preview.plan, build(Some(&lawn), &stations, &[], &[]));
    let seen: Vec<_> = preview
        .nearby
        .iter()
        .map(|n| (n.station_ref, n.network, n.elevation_difference_ft))
        .collect();
    assert_eq!(
        seen,
        vec![
            (2, Network::Uscrn, Some(-50.0)),
            (1, Network::Scan, Some(1500.0))
        ]
    );
    assert!((preview.nearby[0].distance_miles - 20.0).abs() < 0.05);
    assert_eq!(preview.nearby[0].inputs, all(&[&SOIL, &AIR]));
    let unknown = Lawn {
        elevation_ft: None,
        ..LAWN
    };
    assert_eq!(
        super::preview(&unknown, &stations).nearby[0].elevation_difference_ft,
        None
    );
}
