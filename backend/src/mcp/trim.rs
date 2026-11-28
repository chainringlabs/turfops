//! Pure reshaping of REST responses into model-sized tool results: chart series and
//! other bulk the model does not need are dropped, the answers are kept.

use serde_json::{json, Map, Value};

/// Drop the 3-hourly forecast from an `EnvironmentalSummary`; the daily summaries stay.
pub fn drop_hourly_forecast(summary: &mut Value) {
    if let Some(forecast) = summary.get_mut("forecast").and_then(Value::as_object_mut) {
        forecast.remove("hourly");
    }
}

/// `value` without the named top-level keys.
pub fn without(mut value: Value, keys: &[&str]) -> Value {
    if let Some(obj) = value.as_object_mut() {
        for key in keys {
            obj.remove(*key);
        }
    }
    value
}

/// A `TimingResponse` without the daily soil chart series.
pub fn timing_without_series(timing: Value) -> Value {
    without(timing, &["series"])
}

/// A `NitrogenBudget` without its per-application list.
pub fn nitrogen_totals(budget: Value) -> Value {
    without(budget, &["applications"])
}

/// A `DiseaseRiskResponse` reduced to one line per disease: the tier and what to do.
pub fn disease_headlines(risk: &Value) -> Value {
    disease_list(risk, disease_headline)
}

/// A `DiseaseRiskResponse` with each disease's assessment and the recommended
/// preventative, but without the daily series, methodology and full programs.
pub fn disease_overview(risk: &Value) -> Value {
    disease_list(risk, disease_summary)
}

/// One disease in full (series, programs, every fungicide option), by slug.
pub fn disease_detail(risk: &Value, slug: &str) -> Option<Value> {
    let disease = risk
        .get("diseases")?
        .as_array()?
        .iter()
        .find(|d| d.get("slug").and_then(Value::as_str) == Some(slug))?;
    Some(json!({
        "today": risk.get("today"),
        "disease": disease,
        "data_notes": risk.get("data_notes"),
    }))
}

/// The slugs present in a `DiseaseRiskResponse`, for an unknown-slug error.
pub fn disease_slugs(risk: &Value) -> Vec<String> {
    risk.get("diseases")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|d| d.get("slug").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn disease_list(risk: &Value, shape: fn(&Value) -> Value) -> Value {
    let diseases: Vec<Value> = risk
        .get("diseases")
        .and_then(Value::as_array)
        .map(|list| list.iter().map(shape).collect())
        .unwrap_or_default();
    json!({
        "today": risk.get("today"),
        "diseases": diseases,
        "data_notes": risk.get("data_notes"),
    })
}

fn disease_headline(disease: &Value) -> Value {
    let management = disease.get("management");
    json!({
        "slug": disease.get("slug"),
        "name": disease.get("name"),
        "tier": disease.get("tier"),
        "tier_note": disease.get("tier_note"),
        "score_label": disease.get("score_label"),
        "as_of": disease.get("as_of"),
        "action": pick(management, "action"),
        "headline": pick(management, "headline"),
        "protected_through": pick(management.and_then(|m| m.get("protection")), "protected_through"),
    })
}

fn disease_summary(disease: &Value) -> Value {
    let management = disease.get("management");
    let recommended = management
        .and_then(|m| m.get("preventative"))
        .and_then(|p| p.get("options"))
        .and_then(Value::as_array)
        .and_then(|options| {
            options
                .iter()
                .find(|o| o.get("recommended").and_then(Value::as_bool) == Some(true))
        });
    json!({
        "slug": disease.get("slug"),
        "name": disease.get("name"),
        "tier": disease.get("tier"),
        "tier_note": disease.get("tier_note"),
        "score_label": disease.get("score_label"),
        "as_of": disease.get("as_of"),
        "summary": disease.get("summary"),
        "factors": disease.get("factors"),
        "action": pick(management, "action"),
        "headline": pick(management, "headline"),
        "protection": pick(management, "protection"),
        "notes": pick(management, "notes"),
        "recommended_preventative": recommended,
    })
}

fn pick(value: Option<&Value>, key: &str) -> Option<Value> {
    value.and_then(|v| v.get(key)).cloned()
}

/// One part of a composite result: the value, or `null` plus a note saying what is missing.
pub fn part(result: Result<Value, String>, name: &str, notes: &mut Vec<String>) -> Value {
    result.unwrap_or_else(|err| {
        notes.push(format!("{name} unavailable: {err}"));
        Value::Null
    })
}

/// The snapshot's `data_sources`: the connection status, or `null` with a note.
// @spec MCP-SNAP-010
pub fn data_sources(status: Result<Value, serde_json::Error>, notes: &mut Vec<String>) -> Value {
    status.unwrap_or_else(|err| {
        tracing::error!(error = %err, "MCP lawn_snapshot: connection status not serializable");
        notes.push(format!(
            "data_sources unavailable: {}",
            super::tools::INTERNAL_ERROR
        ));
        Value::Null
    })
}

// @spec MCP-TOOL-029
/// Removes every `latitude` and `longitude`, at any depth: the lawn's coordinates never
/// leave the app through a tool. Place names, timezones and distances stay.
pub fn without_coordinates(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("latitude");
            map.remove("longitude");
            map.values_mut().for_each(without_coordinates);
        }
        Value::Array(items) => items.iter_mut().for_each(without_coordinates),
        _ => {}
    }
}

// @spec MCP-TOOL-028
/// The Sources page's answer for the model: per input the basis in use, what stands
/// behind it, the newest observation and the estimate's accuracy; per source its status,
/// history and attribution; the forecast provider and its status. Connections, which
/// credentials are stored, pins and ids are dropped (coordinates go in `json_result`).
pub fn sources(mut view: Value) -> Value {
    if let Some(map) = view.as_object_mut() {
        map.remove("connections");
    }
    for input in view["inputs"].as_array_mut().into_iter().flatten() {
        if let Some(map) = input.as_object_mut() {
            map.remove("pinned_station_ref");
        }
    }
    for source in view["sources"].as_array_mut().into_iter().flatten() {
        if let Some(map) = source.as_object_mut() {
            map.remove("id");
            map.remove("missing_sensor");
        }
    }
    view
}

// @spec MCP-SNAP-005
/// Data sync's source status as the snapshot carries it: per source in use its name,
/// kind, whether its last scheduled fetch succeeded and when, and its inputs.
pub fn source_status(sources: Value) -> Value {
    let newest = |a: &Value, b: &Value| match (a.as_str(), b.as_str()) {
        (Some(x), Some(y)) => Value::from(x.max(y)),
        (Some(x), None) => Value::from(x),
        (None, Some(y)) => Value::from(y),
        (None, None) => Value::Null,
    };
    Value::Array(
        sources
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| {
                serde_json::json!({
                    "name": s["name"],
                    "kind": s["kind"],
                    "ok": s["ok"],
                    "fetched_at": newest(&s["last_success"], &s["last_failure"]),
                    "inputs": s["inputs"],
                })
            })
            .collect(),
    )
}

/// Build an object from named parts.
pub fn object(parts: Vec<(&str, Value)>) -> Value {
    Value::Object(
        parts
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect::<Map<_, _>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // @spec MCP-SNAP-010
    #[test]
    fn source_status_that_will_not_serialize_is_null_with_a_note() {
        let mut notes = Vec::new();
        let err = serde_json::from_str::<Value>("{").unwrap_err();
        assert_eq!(data_sources(Err(err), &mut notes), Value::Null);
        assert_eq!(
            notes,
            vec![format!(
                "data_sources unavailable: {}",
                crate::mcp::tools::INTERNAL_ERROR
            )]
        );
    }

    // @spec MCP-SNAP-005
    #[test]
    fn source_status_is_name_kind_ok_when_and_inputs() {
        let status = serde_json::json!([{
            "id": 4, "name": "Avondale 2 N", "kind": "Station", "ok": true,
            "last_success": "2026-11-25T12:00:00Z", "last_failure": "2026-11-24T12:00:00Z",
            "last_error": null, "inputs": ["soil_temp_5"], "held_from": "2011-01-01",
            "held_through": "2026-11-24", "wanted_from": "2011-01-01"
        }]);
        assert_eq!(
            source_status(status),
            serde_json::json!([{
                "name": "Avondale 2 N", "kind": "Station", "ok": true,
                "fetched_at": "2026-11-25T12:00:00Z", "inputs": ["soil_temp_5"]
            }])
        );
    }

    #[test]
    fn summary_drops_only_the_hourly_forecast() {
        let mut summary = json!({
            "soil_temp_7day_avg_f": 68.2,
            "forecast": {"hourly": [1, 2, 3], "daily_summary": [{"date": "2026-09-22"}]}
        });
        drop_hourly_forecast(&mut summary);
        assert!(summary["forecast"].get("hourly").is_none());
        assert_eq!(
            summary["forecast"]["daily_summary"][0]["date"],
            "2026-09-22"
        );
        assert_eq!(summary["soil_temp_7day_avg_f"], 68.2);
    }

    #[test]
    fn summary_without_forecast_is_untouched() {
        let mut summary = json!({"soil_temp_7day_avg_f": 60.0});
        drop_hourly_forecast(&mut summary);
        assert_eq!(summary, json!({"soil_temp_7day_avg_f": 60.0}));
    }

    #[test]
    fn timing_drops_series_keeps_windows() {
        let trimmed = timing_without_series(json!({
            "windows": [{"id": "FallSeeding", "state": "Ideal"}],
            "series": [{"date": "2026-09-01"}],
            "data_notes": []
        }));
        assert!(trimmed.get("series").is_none());
        assert_eq!(trimmed["windows"][0]["state"], "Ideal");
    }

    #[test]
    fn disease_headlines_keep_tier_action_and_protection() {
        let risk = json!({
            "today": "2026-09-22",
            "station": "PA Avondale",
            "data_notes": ["the record lags a day"],
            "diseases": [{
                "slug": "brown-patch", "name": "Brown patch", "tier": "High",
                "tier_note": null, "score": 5.4, "score_label": "E-index 5.4",
                "as_of": "2026-09-21", "daily": [{"date": "2026-09-20"}],
                "methodology": {"model": "Fidanza"},
                "management": {
                    "action": "Protected", "headline": "Covered by azoxystrobin",
                    "cultural": ["water early"],
                    "protection": {"protected_through": "2026-10-01"}
                }
            }]
        });
        let trimmed = disease_headlines(&risk);
        let d = &trimmed["diseases"][0];
        assert_eq!(d["slug"], "brown-patch");
        assert_eq!(d["tier"], "High");
        assert_eq!(d["action"], "Protected");
        assert_eq!(d["protected_through"], "2026-10-01");
        assert!(d.get("daily").is_none());
        assert!(d.get("methodology").is_none());
        assert!(d.get("cultural").is_none());
        assert_eq!(trimmed["data_notes"][0], "the record lags a day");
    }

    #[test]
    fn disease_headline_without_protection_is_null() {
        let trimmed = disease_headlines(&json!({
            "diseases": [{"slug": "dollar-spot", "management": {"action": "Monitor", "protection": null}}]
        }));
        assert_eq!(trimmed["diseases"][0]["protected_through"], Value::Null);
    }

    #[test]
    fn nitrogen_totals_drop_the_application_list() {
        let trimmed = nitrogen_totals(json!({
            "year": 2026, "remaining_lbs_per_1000sqft": 1.5, "applications": [{}]
        }));
        assert!(trimmed.get("applications").is_none());
        assert_eq!(trimmed["remaining_lbs_per_1000sqft"], 1.5);
    }

    #[test]
    fn failed_part_is_null_with_a_note() {
        let mut notes = Vec::new();
        let ok = part(Ok(json!(1)), "timing", &mut notes);
        let failed = part(Err("record down".into()), "disease_risk", &mut notes);
        assert_eq!(ok, json!(1));
        assert_eq!(failed, Value::Null);
        assert_eq!(notes, vec!["disease_risk unavailable: record down"]);
    }

    fn full_risk() -> Value {
        json!({
            "today": "2026-09-22",
            "data_notes": [],
            "diseases": [{
                "slug": "dollar-spot", "name": "Dollar spot", "tier": "High",
                "summary": "Warm humid nights", "factors": [{"label": "RH", "value": "88%"}],
                "daily": [{"date": "2026-09-21"}], "methodology": {"model": "Smith-Kerns"},
                "scale": {"max": 100},
                "management": {
                    "action": "ApplyPreventative", "headline": "Apply FRAC 3",
                    "cultural": ["remove dew"], "notes": ["label governs"], "protection": null,
                    "preventative": {"options": [
                        {"frac_class": "Frac7", "recommended": false},
                        {"frac_class": "Frac3", "recommended": true, "on_hand": [{"name": "Propiconazole"}]}
                    ]},
                    "curative": {"options": []}
                }
            }]
        })
    }

    #[test]
    fn disease_overview_keeps_assessment_and_the_recommended_option_only() {
        let overview = disease_overview(&full_risk());
        let d = &overview["diseases"][0];
        assert_eq!(d["summary"], "Warm humid nights");
        assert_eq!(d["factors"][0]["value"], "88%");
        assert_eq!(d["recommended_preventative"]["frac_class"], "Frac3");
        assert_eq!(
            d["recommended_preventative"]["on_hand"][0]["name"],
            "Propiconazole"
        );
        for dropped in ["daily", "methodology", "scale", "cultural", "curative"] {
            assert!(d.get(dropped).is_none(), "{dropped} should be dropped");
        }
    }

    #[test]
    fn disease_detail_returns_everything_for_a_known_slug() {
        let risk = full_risk();
        let detail = disease_detail(&risk, "dollar-spot").unwrap();
        assert_eq!(detail["disease"]["daily"][0]["date"], "2026-09-21");
        assert_eq!(detail["disease"]["management"]["cultural"][0], "remove dew");
        assert!(disease_detail(&risk, "brown-patch").is_none());
        assert_eq!(disease_slugs(&risk), vec!["dollar-spot"]);
    }

    // @spec MCP-SNAP-010
    #[test]
    fn unserializable_connection_status_is_null_with_a_note() {
        let mut notes = Vec::new();
        let ok = data_sources(Ok(json!({"soildata": true})), &mut notes);
        assert_eq!(ok, json!({"soildata": true}));
        assert!(notes.is_empty());
        let err = serde_json::from_str::<Value>("{").unwrap_err();
        assert_eq!(data_sources(Err(err), &mut notes), Value::Null);
        assert_eq!(
            notes,
            vec![
                "data_sources unavailable: TurfOps hit an internal error answering this; \
                  details are in the server log."
            ]
        );
    }

    #[test]
    fn without_drops_only_the_named_keys() {
        let v = without(json!({"a": 1, "b": 2, "c": 3}), &["a", "c"]);
        assert_eq!(v, json!({"b": 2}));
    }
}
