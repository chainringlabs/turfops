//! What the doctrine (the server instructions) tells a model.

use super::instructions::INSTRUCTIONS;

fn says(phrases: &[&str]) {
    for phrase in phrases {
        assert!(INSTRUCTIONS.contains(phrase), "doctrine lacks {phrase:?}");
    }
}

// @spec MCP-DOC-001
#[test]
fn the_doctrine_is_static_and_leaves_the_lawns_particulars_to_the_profile() {
    let doctrine: &'static str = INSTRUCTIONS;
    // How to use TurfOps, not a description of one lawn: no size, soil or irrigation.
    let lower = doctrine.to_lowercase();
    for particular in ["irrigat", "clay", "loam", "sandy", "acre", "lawn_size"] {
        assert!(!lower.contains(particular), "states {particular:?}");
    }
    let sizes = doctrine
        .match_indices("sq ft")
        .filter(|(i, _)| !doctrine[..*i].trim_end().ends_with("1,000"));
    assert_eq!(sizes.count(), 0, "states a lawn size");
    // The grass is named only as the cool-season family, not this lawn's species.
    says(&["cool-season lawn (tall fescue / Kentucky bluegrass / perennial rye / a Mixed blend)"]);
    // The particulars arrive with the snapshot, whose first part is the profile.
    says(&["call `lawn_snapshot`"]);
}

// @spec MCP-DOC-002
#[test]
fn the_doctrine_sends_the_model_to_the_snapshot_first_and_forbids_guessing() {
    says(&[
        "Before answering any question about what to do on \
this lawn now or this season, call `lawn_snapshot`; then fetch detail with the narrower \
tools.",
        "Never guess a reading, date or product that a tool can give you.",
        "`turfops://docs/agronomy` and `turfops://docs/timing-windows`",
        "read them when the user asks why TurfOps says what it does",
    ]);
}

// @spec MCP-DOC-003
#[test]
fn the_doctrine_describes_the_setting() {
    says(&[
        "in the contiguous United States; where it is and which grass it has come from \
`lawn_profile`",
        "`today` in the responses is the lawn's local date",
        "Keep observed values \
and forecast values distinct",
        "Temperatures are Fahrenheit",
        "Soil timing uses the 5 cm depth; the live soil reading \
and the grub window use 10 cm",
    ]);
    // Nothing about one place, station or provider.
    for particular in [
        "southeast Pennsylvania",
        "USCRN",
        "OpenWeatherMap",
        "lags about a day",
    ] {
        assert!(!INSTRUCTIONS.contains(particular), "names {particular:?}");
    }
}

// @spec MCP-DOC-012
#[test]
fn the_doctrine_says_every_value_has_a_basis_to_relay() {
    says(&[
        "Every weather value has a basis: measured at a named station (with its distance), the \
owner's sensor, or estimated for the lawn's location. Say which when you give a reading \
or a date that rests on one, and never present an estimate as a measurement.",
        "An estimated soil date comes with its typical error (`typical_error_days`); relay it.",
        "A \
disease tier marked `approximate` rests on an hourly estimate of air and humidity; say so.",
        "Soil moisture may have no source for this location: say that, never infer a value.",
        "Call `sources` to answer where any value comes from",
    ]);
}

// @spec MCP-DOC-006
#[test]
fn the_doctrine_forbids_invented_products_and_brands_off_the_shelf() {
    says(&[
        "Never invent products.",
        "Prefer what is on the shelf",
        "When suggesting chemistry that is not on the shelf, name the \
active ingredient and, for fungicides, the FRAC class — not a brand.",
    ]);
}

// @spec MCP-DOC-007
#[test]
fn the_doctrine_keeps_seed_and_pre_emergent_apart() {
    says(&[
        "Seeding and pre-emergent never share a season.",
        "`Blocked`",
        "Fall pre-emergent is the alternative to fall seeding",
        "Core aeration belongs to the fall seeding window.",
    ]);
}

// @spec MCP-DOC-008
#[test]
fn the_doctrine_names_window_states_date_sources_and_the_null_date() {
    for state in [
        "NotYet",
        "OpeningSoon",
        "Open,",
        "Ideal",
        "Closing",
        "Closed",
        "Done",
        "Blocked",
    ] {
        says(&[state]);
    }
    for source in ["Observed", "Tentative", "Forecast (", "Typical"] {
        says(&[source]);
    }
    says(&[
        "A boundary with `date: null` means \"later than usual — not yet seen\", not unknown.",
        "Do \
not invent a date for it.",
    ]);
}

// @spec MCP-DOC-009
#[test]
fn the_doctrine_explains_disease_tiers_protection_and_curatives() {
    says(&[
        "only the Low / Moderate / High / \
Severe tier is comparable across diseases",
        "`Protected` means a logged fungicide rated \
Good or better on that disease is still inside its residual window",
        "Red thread's remedy \
is nitrogen, not a spray",
        "TurfOps \
has no symptom input, so ask the owner what they see before recommending a curative",
        "Rotate FRAC classes",
        "Call `disease_detail` before naming a fungicide",
    ]);
}

// @spec MCP-DOC-010
#[test]
fn the_doctrine_says_to_cite_active_recommendations_and_reads_inventory_state() {
    says(&[
        "cite it (title and id) and its reasoning rather than re-deriving it",
        "OnHand (the shelf covers it)",
        "Low (only Low-stock products cover \
it)",
        "NotOnHand (nothing on the shelf fits)",
        "Partial (some needs covered, some missing)",
        "come back when they escalate or when \
their episode ends; `include_inactive` shows them",
    ]);
}
