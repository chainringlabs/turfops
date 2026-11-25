use crate::db::{product_queries, queries};
use crate::error::TurfOpsError;
use crate::logic::data_sync::record_asked;
use crate::logic::disease::{self, weather_days};
use crate::logic::weather_record::record::WeatherRecord;
use crate::models::weather_basis::{Basis, BasisKind, BasisSpan};
use crate::models::{
    DailyWeather, Disease, DiseaseContext, DiseaseRiskResponse, LawnProfile, ProductCategory,
    Recommendation, RiskBasis, ShelfFungicide, ShelfProduct, StockStatus, AFTERNOON_HOURS,
    DAWN_HOURS,
};
use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use chrono::{Duration, NaiveDate};

// @spec DIS-MODEL-API-001, DIS-MODEL-API-002
/// GET /api/v1/disease-risk
/// Per-disease risk from published/heuristic weather models, each with a management
/// plan driven by the lawn's application history.
pub async fn get_disease_risk(
    State(state): State<AppState>,
) -> Result<Json<DiseaseRiskResponse>, TurfOpsError> {
    let profile = queries::get_default_lawn_profile(&state.pool)
        .await?
        .ok_or_else(|| TurfOpsError::NotFound("No lawn profile found".into()))?;
    let (response, _) = compute(&state, &profile).await?;
    Ok(Json(response))
}

// @spec FEED-PIPE-015, DIS-MGMT-REC-005
/// Disease recommendations for the feed/dashboard. Disease risk is additive there, so
/// a weather record outage degrades to "no disease alerts" rather than failing.
pub(crate) async fn recommendations(
    state: &AppState,
    profile: &LawnProfile,
) -> Vec<Recommendation> {
    match compute(state, profile).await {
        Ok((response, ctx)) => {
            disease::recommendations::to_recommendations(&response.diseases, &ctx)
        }
        Err(e) => {
            tracing::warn!("Disease risk unavailable for recommendations: {}", e);
            Vec::new()
        }
    }
}

/// Observed days come from the weather record's hourly air and humidity, today's
/// remainder and the outlook from the forecast the schedule holds, and lawn history
/// (overseeding, nitrogen, fungicides) from the applications log. Returns the context
/// alongside the response so the recommendation bridge can see the same history.
async fn compute(
    state: &AppState,
    profile: &LawnProfile,
) -> Result<(DiseaseRiskResponse, DiseaseContext), TurfOpsError> {
    // @spec DIS-MODEL-DAY-001
    let asked = record_asked(&state.pool, &state.clock).await?;
    let today = asked.at.today;
    let fetch_start = today - Duration::days(disease::LOOKBACK_DAYS);
    // @spec DIS-MODEL-DAY-002
    let observed = WeatherRecord::new(state.pool.clone())
        .fetch_daily_disease_inputs(fetch_start, DAWN_HOURS, AFTERNOON_HOURS, asked)
        .await?;
    let forecast = {
        let mut service = state.sync_service.write().await;
        service.get_or_refresh().await?.forecast
    };

    // @spec DIS-MODEL-DAY-003, DIS-MODEL-DAY-007
    let forecast_days = forecast
        .as_ref()
        .map(|f| weather_days::forecast_days(&f.hourly, &state.clock, observed.latest_observed))
        .unwrap_or_default();
    let series = weather_days::build_series(observed.days, forecast_days);

    let profile_id = profile
        .id
        .ok_or_else(|| TurfOpsError::InvalidData("Profile missing ID".into()))?;
    // Its own range, so the feed and this page judge the same log.
    // @spec FEED-PIPE-011
    let history = queries::get_applications_for_profile_in_range(
        &state.pool,
        profile_id,
        today - Duration::days(365),
        today + Duration::days(1),
    )
    .await?;
    let mut ctx = disease::context_from_history(&history, today);
    // @spec DIS-MGMT-PICK-005
    ctx.shelf = product_queries::list_products_for_profile(
        &state.pool,
        profile_id,
        Some(ProductCategory::Fungicide),
        false,
    )
    .await?
    .iter()
    .filter(|p| p.stock_status != StockStatus::Out)
    .filter_map(|p| {
        Some(ShelfFungicide {
            product: ShelfProduct::of(p)?,
            classes: p.facts.frac_classes.clone(),
        })
    })
    .collect();

    // Every tracked disease is a cool-season turf disease.
    let cool_season = profile.grass_type.is_cool_season();
    let diseases = if cool_season {
        disease::assess_all(&series, today, &ctx)
    } else {
        Vec::new()
    };
    let omitted: Vec<Disease> = if cool_season {
        Disease::ALL
            .into_iter()
            .filter(|d| !diseases.iter().any(|r| r.disease == *d))
            .collect()
    } else {
        Vec::new()
    };
    let spans = display_spans(&observed.basis, today);
    let mut data_notes = data_notes(&series, &spans, today, forecast.is_some(), &omitted);
    if !cool_season {
        data_notes.push("Disease models cover cool-season turf only.".into());
    }

    let headline = disease::headline_day(&series, today);
    let response = DiseaseRiskResponse {
        generated_at: asked.at.now,
        today,
        basis: RiskBasis {
            headline: headline.and_then(|day| day.basis.clone()),
            spans,
        },
        approximate: headline.is_some_and(DailyWeather::is_approximate),
        diseases,
        data_notes,
    };
    Ok((response, ctx))
}

/// The basis spans that fall in the display window's observed days, clamped to it.
fn display_spans(spans: &[BasisSpan], today: NaiveDate) -> Vec<BasisSpan> {
    let first = today - Duration::days(disease::HISTORY_DAYS);
    spans
        .iter()
        .filter(|span| span.to >= first && span.from <= today)
        .map(|span| BasisSpan {
            from: span.from.max(first),
            to: span.to.min(today),
            ..span.clone()
        })
        .collect()
}

/// The note given when observed days rest on the hourly analysis.
const APPROXIMATE_NOTE: &str = "Air temperature and humidity on the approximate days are an \
     hourly estimate for your location, not a measurement at the lawn. Against measured \
     stations it counts humid hours within about 3 hours a day. A sensor at the lawn would \
     replace it: map one on the Sources page.";

// @spec DIS-MODEL-API-003, DIS-MODEL-API-007, DIS-MODEL-API-008
/// Plain-language caveats about the inputs behind this response. `spans` are the basis
/// spans over the display window.
fn data_notes(
    series: &[DailyWeather],
    spans: &[BasisSpan],
    today: NaiveDate,
    has_forecast: bool,
    omitted: &[Disease],
) -> Vec<String> {
    // @spec DIS-MODEL-API-004, DIS-MODEL-API-005
    // "Usable" is the headline's own rule (hours, dawn and afternoon), so a note explains
    // every fallback the headline makes.
    let mut notes = Vec::new();
    let latest_observed = series
        .iter()
        .filter(|d| !d.is_forecast && disease::is_usable(d))
        .map(|d| d.date)
        .max();
    match latest_observed {
        None => notes.push("No recent weather observations are available.".into()),
        Some(date) if date < today - Duration::days(1) => notes.push(format!(
            "Weather observations end {}; more recent days rely on the forecast.",
            date.format("%b %-d")
        )),
        Some(_) => {}
    }

    let first = today - Duration::days(disease::HISTORY_DAYS);
    if series
        .iter()
        .any(|d| d.date >= first && d.date <= today && d.is_approximate())
    {
        notes.push(APPROXIMATE_NOTE.into());
    }
    for pair in spans.windows(2) {
        if pair[0].basis != pair[1].basis {
            notes.push(format!(
                "From {}, air temperature and humidity come from {} instead of {}.",
                pair[1].from.format("%b %-d"),
                described(&pair[1].basis),
                described(&pair[0].basis)
            ));
        }
    }

    let today_usable = series
        .iter()
        .any(|d| d.date == today && disease::is_usable(d));
    if !today_usable {
        notes.push(
            "Today does not yet have enough recorded and forecast hours, so current risk \
             reflects the most recent complete day."
                .into(),
        );
    }
    if !has_forecast {
        notes.push("Forecast unavailable — no outlook days are shown.".into());
    }
    for d in omitted {
        notes.push(format!(
            "{} is left out: no recent day has enough weather data to score it.",
            sentence_case(d.name())
        ));
    }
    notes
}

/// A basis in a sentence: "Avondale 2 N (20 mi)", "your sensor Patio", "the hourly
/// estimate for your location".
fn described(basis: &Basis) -> String {
    match basis.kind {
        BasisKind::Station => match basis.distance_miles {
            Some(miles) => format!("{} ({miles:.0} mi)", basis.source),
            None => basis.source.clone(),
        },
        BasisKind::Sensor => format!("your sensor {}", basis.source),
        BasisKind::Estimate => "the hourly estimate for your location".into(),
    }
}

/// "Gray Leaf Spot" → "Gray leaf spot".
fn sentence_case(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(all(test, feature = "db-tests"))]
mod db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod shelf_db_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::disease::test_support::{date, day, run_of};
    use crate::models::weather_record::Input;

    fn station() -> Basis {
        Basis {
            kind: BasisKind::Station,
            source: "Avondale 2 N".into(),
            distance_miles: Some(6.2),
            approximate: false,
        }
    }

    fn analysis() -> Basis {
        Basis {
            kind: BasisKind::Estimate,
            source: "Hourly analysis".into(),
            distance_miles: None,
            approximate: true,
        }
    }

    fn span(from: NaiveDate, to: NaiveDate, basis: Basis) -> BasisSpan {
        BasisSpan {
            input: Input::AirTemp,
            from,
            to,
            basis,
        }
    }

    fn notes(series: &[DailyWeather], today: NaiveDate, has_forecast: bool) -> Vec<String> {
        data_notes(series, &[], today, has_forecast, &[])
    }

    #[test]
    fn no_notes_when_today_is_covered() {
        let today = date(9, 20);
        let mut series = run_of(today, 3, |d| day(d, 20.0, 15.0, 25.0, 80.0));
        series[2].is_forecast = true;
        assert!(notes(&series, today, true).is_empty());
    }

    // @spec DIS-MODEL-API-003
    #[test]
    fn notes_flag_lagging_observations_thin_today_and_missing_forecast() {
        let today = date(9, 20);
        let series = run_of(date(9, 17), 3, |d| day(d, 20.0, 15.0, 25.0, 80.0));
        let notes = notes(&series, today, false);
        assert_eq!(
            notes,
            vec![
                "Weather observations end Sep 17; more recent days rely on the forecast.",
                "Today does not yet have enough recorded and forecast hours, so current risk \
                 reflects the most recent complete day.",
                "Forecast unavailable — no outlook days are shown.",
            ]
        );
    }

    // @spec DIS-MODEL-API-004
    #[test]
    fn today_without_dawn_hours_is_not_usable_for_the_notes_either() {
        let today = date(9, 20);
        let mut series = run_of(today, 3, |d| day(d, 20.0, 15.0, 25.0, 80.0));
        // Twelve hours of today, but none of them dawn: the headline falls back.
        series[2].hours_covered = 12.0;
        series[2].covers_dawn = false;
        let notes = notes(&series, today, true);
        assert!(
            notes.iter().any(|n| n.contains("most recent complete day")),
            "{notes:?}"
        );
    }

    // @spec DIS-MODEL-API-005
    #[test]
    fn a_disease_left_out_is_named() {
        let today = date(9, 20);
        let series = run_of(today, 3, |d| day(d, 20.0, 15.0, 25.0, 80.0));
        let notes = data_notes(&series, &[], today, true, &[Disease::GrayLeafSpot]);
        assert_eq!(
            notes,
            vec!["Gray leaf spot is left out: no recent day has enough weather data to score it."]
        );
    }

    // @spec DIS-MODEL-API-003
    #[test]
    fn no_observed_days_at_all_is_its_own_note() {
        let today = date(9, 20);
        let mut forecast_only = run_of(date(9, 22), 3, |d| day(d, 20.0, 15.0, 25.0, 80.0));
        for d in &mut forecast_only {
            d.is_forecast = true;
        }
        assert_eq!(
            notes(&forecast_only, today, true),
            vec!["No recent weather observations are available."]
        );
    }

    // @spec DIS-MODEL-API-007
    #[test]
    fn an_approximate_day_in_the_window_brings_the_estimate_note() {
        let today = date(9, 20);
        let mut series = run_of(today, 10, |d| day(d, 20.0, 15.0, 25.0, 80.0));
        // Too old to be shown: no note.
        series[0].basis = Some(analysis());
        assert!(notes(&series, today, true).is_empty());

        series[8].basis = Some(analysis());
        let got = notes(&series, today, true);
        assert_eq!(got, vec![APPROXIMATE_NOTE]);
        assert!(got[0].contains("hourly estimate for your location"));
        assert!(got[0].contains("within about 3 hours a day"));
        assert!(got[0].contains("A sensor at the lawn would replace it"));
    }

    // @spec DIS-MODEL-API-008
    #[test]
    fn a_change_of_basis_in_the_window_is_dated() {
        let today = date(9, 20);
        let series = run_of(today, 3, |d| day(d, 20.0, 15.0, 25.0, 80.0));
        let spans = [
            span(date(9, 14), date(9, 17), station()),
            span(date(9, 18), date(9, 20), analysis()),
        ];
        let notes = data_notes(&series, &spans, today, true, &[]);
        assert_eq!(
            notes,
            vec![
                "From Sep 18, air temperature and humidity come from the hourly estimate for \
                 your location instead of Avondale 2 N (6 mi)."
            ]
        );
        let same = [
            span(date(9, 14), date(9, 15), station()),
            span(date(9, 17), date(9, 20), station()),
        ];
        assert!(data_notes(&series, &same, today, true, &[]).is_empty());
    }

    // @spec DIS-MODEL-API-001
    #[test]
    fn the_spans_shown_are_the_display_windows_clamped_to_it() {
        let today = date(9, 20);
        let spans = [
            span(date(9, 1), date(9, 10), station()),
            span(date(9, 11), date(9, 16), station()),
            span(date(9, 17), date(9, 20), analysis()),
        ];
        let shown = display_spans(&spans, today);
        assert_eq!(shown.len(), 2);
        assert_eq!((shown[0].from, shown[0].to), (date(9, 14), date(9, 16)));
        assert_eq!((shown[1].from, shown[1].to), (date(9, 17), date(9, 20)));
    }
}
