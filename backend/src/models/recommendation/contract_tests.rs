//! The shape every surface receives and the severity order every comparison uses.

use super::*;
use crate::models::{InventoryState, InventoryStatus, ProductCategory, ProductNeed};

fn sample() -> Recommendation {
    Recommendation::new(
        "grub_control_2026",
        RecommendationCategory::GrubControl,
        Severity::Advisory,
        "Grub preventative window",
        "Apply a grub preventative.",
    )
    .with_explanation("Egg-laying is under way.")
    .with_data_point("GDD", 600, "Calculated")
    .with_action("Apply before peak hatch.")
    .with_need(ProductNeed::new(
        "grub preventative",
        ProductCategory::InsectControl,
    ))
}

// @spec FEED-REC-001
#[test]
fn a_recommendation_carries_every_field_a_surface_shows() {
    let mut rec = sample().with_feeding(0.5);
    rec.inventory = Some(InventoryStatus {
        state: InventoryState::NotOnHand,
        matches: vec![],
        missing: vec!["grub preventative".into()],
    });
    let json = serde_json::to_value(&rec).unwrap();
    let mut keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    let mut expected = vec![
        "id",
        "category",
        "severity",
        "title",
        "description",
        "explanation",
        "data_points",
        "suggested_action",
        "needs",
        "inventory",
        "created_at",
        "dismissed",
        "addressed",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected, "and nothing internal (the feeding mark)");
    assert_eq!(json["id"], "grub_control_2026");
    assert_eq!(json["category"], "GrubControl");
    assert_eq!(json["severity"], "Advisory");
    assert_eq!(json["explanation"], "Egg-laying is under way.");
    assert_eq!(json["data_points"][0]["label"], "GDD");
    assert_eq!(json["data_points"][0]["value"], "600");
    assert_eq!(json["suggested_action"], "Apply before peak hatch.");
    assert_eq!(json["inventory"]["state"], "NotOnHand");
    assert_eq!(json["dismissed"], false);
    assert_eq!(json["addressed"], false);
}

// @spec FEED-REC-002
#[test]
fn severities_order_info_advisory_warning_critical() {
    let mut shuffled = vec![
        Severity::Warning,
        Severity::Info,
        Severity::Critical,
        Severity::Advisory,
    ];
    shuffled.sort();
    assert_eq!(
        shuffled,
        vec![
            Severity::Info,
            Severity::Advisory,
            Severity::Warning,
            Severity::Critical
        ]
    );
    assert!(Severity::Info < Severity::Advisory);
    assert!(Severity::Advisory < Severity::Warning);
    assert!(Severity::Warning < Severity::Critical);
}

// @spec FEED-REC-003
#[test]
fn needs_are_always_sent_and_inventory_only_when_set() {
    let plain = Recommendation::new(
        "rain_delay",
        RecommendationCategory::ApplicationTiming,
        Severity::Info,
        "t",
        "d",
    );
    let json = serde_json::to_value(&plain).unwrap();
    assert_eq!(json["needs"], serde_json::json!([]));
    assert!(json.get("inventory").is_none());

    let json = serde_json::to_value(sample()).unwrap();
    assert_eq!(json["needs"].as_array().unwrap().len(), 1);
    assert!(json.get("inventory").is_none(), "no verdict yet");
}

// @spec FEED-REC-005, WX-VIEW-BASIS-001
#[test]
fn a_weather_point_carries_its_basis_and_names_it() {
    let station = Basis {
        kind: crate::models::weather_basis::BasisKind::Station,
        source: "Avondale 2 N".into(),
        distance_miles: Some(20.0),
        approximate: false,
    };
    let rec = sample()
        .with_weather_point(
            "Soil",
            "48°F",
            Measured::Record {
                basis: Some(&station),
                mixed: true,
            },
        )
        .with_weather_point("Air", "50°F", Measured::now(None))
        .with_weather_point("Rain", "0.2\"", Measured::Forecast("Open-Meteo"));
    let [_, soil, air, rain] = &rec.data_points[..] else {
        panic!("four points");
    };
    assert_eq!(
        (soil.source.as_str(), soil.basis.as_ref(), soil.mixed),
        ("Avondale 2 N (20 mi)", Some(&station), true)
    );
    assert_eq!(
        (air.source.as_str(), air.basis.as_ref(), air.mixed),
        (RECORD_SOURCE, None, false)
    );
    assert_eq!(
        (rain.source.as_str(), rain.basis.as_ref()),
        ("Open-Meteo", None)
    );

    // Off the wire, a point without a basis has neither field.
    let json = serde_json::to_value(&rec.data_points).unwrap();
    assert!(json[0].get("basis").is_none() && json[0].get("mixed").is_none());
    assert_eq!(json[1]["basis"]["source"], "Avondale 2 N");
    assert_eq!(json[1]["mixed"], true);
}
