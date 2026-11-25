//! The yearly running total of GDD and the crabgrass bands.

use super::*;
use chrono::NaiveDate;

fn d(m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, m, day).unwrap()
}

// @spec RULE-GDD-003
#[test]
fn the_running_total_restarts_every_january_first() {
    let rows = [
        (
            NaiveDate::from_ymd_opt(2025, 12, 30).unwrap(),
            60.0,
            40.0,
            4.0,
        ),
        (
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
            60.0,
            40.0,
            6.0,
        ),
        (d(1, 1), 60.0, 40.0, 1.0),
        (d(1, 2), 60.0, 40.0, 2.0),
    ];
    let totals: Vec<f64> = accumulate_daily_gdd(&rows)
        .iter()
        .map(|g| g.cumulative_gdd_base50)
        .collect();
    assert_eq!(totals, vec![4.0, 10.0, 1.0, 3.0]);
}

// @spec RULE-GDD-004
#[test]
fn crabgrass_status_bands_and_threshold() {
    use CrabgrassStatus::*;
    for (gdd, status) in [
        (0.0, PreGermination),
        (74.9, PreGermination),
        (75.0, ApproachingGermination),
        (149.9, ApproachingGermination),
        (150.0, GerminationLikely),
        (199.9, GerminationLikely),
        (200.0, PostGermination),
        (1500.0, PostGermination),
    ] {
        let model = crabgrass_model(gdd);
        assert_eq!(model.status, status, "{gdd}");
        assert_eq!(model.germination_threshold, 200.0);
        assert_eq!(model.current_gdd, gdd);
    }
}

// @spec RULE-GDD-005
#[test]
fn no_recommendation_source_reads_the_crabgrass_model() {
    // The model lives in logic/gdd.rs and reaches the owner only through `/gdd` (and the
    // MCP tool that calls it). Nothing that builds recommendations may use it.
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let allowed = [
        "logic/gdd.rs",
        "logic/gdd/",
        "models/",
        "api/gdd.rs",
        "api/gdd/",
        "mcp/",
    ];
    let mut stack = vec![src.clone()];
    let mut readers = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(&src)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if !rel.ends_with(".rs") || allowed.iter().any(|a| rel.starts_with(a)) {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            if text.contains("crabgrass_model") || text.contains("CrabgrassStatus") {
                readers.push(rel);
            }
        }
    }
    assert!(readers.is_empty(), "crabgrass model read by {readers:?}");
}
