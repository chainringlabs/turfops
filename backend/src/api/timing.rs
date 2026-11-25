#[cfg(all(test, feature = "db-tests"))]
pub(crate) mod db_harness;
#[cfg(all(test, feature = "db-tests"))]
mod handler_tests;
#[cfg(all(test, feature = "db-tests"))]
mod hook_tests;
mod memo;
#[cfg(test)]
mod memo_tests;

use crate::db::{queries, soil_estimate_queries};
use crate::error::TurfOpsError;
use crate::logic::data_sync::{plan_key, record_asked};
use crate::logic::timing::{self, context, Assessment, SoilEstimate};
use crate::logic::weather_record::record::WeatherRecord;
use crate::models::climate::ClimateRecord;
use crate::models::seasonal_plan::PlannedActivity;
use crate::models::timing::{TimingBasis, TimingResponse};
use crate::models::weather_basis::{Basis, BasisKind, BasisSpan};
use crate::models::weather_record::Input;
use crate::models::{Application, DailyForecast, LawnProfile, Recommendation};
use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use std::sync::Arc;

/// Application history window: reaches the previous July from any date, which is as
/// far back as a window's done/conflict spans look.
const HISTORY_LOOKBACK_DAYS: i64 = 550;
/// Fewer complete years than this and the typical dates are flagged as rough.
const MIN_SOLID_HISTORY_YEARS: usize = 5;
/// Full forecast days the windows use: air forecasts past this carry little skill.
const FORECAST_DAYS: usize = 10;

// @spec TIME-ENG-040, TIME-ENG-041
/// GET /api/v1/timing-windows
/// Seeding and pre-emergent windows from the weather record's 5 cm soil, freeze
/// climatology, the soil forecast and the application log.
pub async fn get_timing_windows(
    State(state): State<AppState>,
) -> Result<Json<TimingResponse>, TurfOpsError> {
    let profile = queries::get_default_lawn_profile(&state.pool)
        .await?
        .ok_or_else(|| TurfOpsError::NotFound("No lawn profile found".into()))?;
    Ok(Json(compute(&state, &profile).await?))
}

// @spec TIME-ENG-015
/// First day of the climate record: Jan 1, `HISTORY_YEARS` before the lawn's current year.
pub(crate) fn climate_start(today: NaiveDate) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(today.year() - timing::HISTORY_YEARS, 1, 1)
}

/// The climate record and, when it could not be re-read, when the record served in its
/// place was read.
pub(crate) struct ClimateRead {
    pub record: Arc<ClimateRecord>,
    pub last_good_at: Option<chrono::DateTime<Utc>>,
}

/// The memoized climate record (see `climate_read`).
pub(crate) async fn climate_record(state: &AppState) -> Result<Arc<ClimateRecord>, TurfOpsError> {
    Ok(climate_read(state).await?.record)
}

/// The memoized climate record, re-read from the weather record when missing, an hour old
/// or read under another basis plan. When the re-read fails, the slot is left as it is
/// and its record stands in while under a day old (and of the same plan, when the plan
/// could be read); only with no such record does the error reach the caller.
// @spec SYNC-MEMO-001, SYNC-MEMO-002, TIME-ENG-038
pub(crate) async fn climate_read(state: &AppState) -> Result<ClimateRead, TurfOpsError> {
    let now = state.clock.now();
    let slot = state.climate_cache.read().await.clone();
    let stand_in = |key: Option<&str>, e: TurfOpsError| match slot.clone().filter(
        |(held, read_at, _)| key.is_none_or(|key| held == key) && memo::may_stand_in(*read_at, now),
    ) {
        Some((_, read_at, record)) => {
            tracing::warn!(%read_at, "Climate record re-read failed; serving the last good read: {e}");
            Ok(ClimateRead {
                record,
                last_good_at: Some(read_at),
            })
        }
        None => Err(e),
    };
    let asked = match record_asked(&state.pool, &state.clock).await {
        Ok(asked) => asked,
        Err(e) => return stand_in(None, e),
    };
    let key = match plan_key(&state.pool).await {
        Ok(key) => key,
        Err(e) => return stand_in(None, e),
    };
    if let Some((held, read_at, record)) = &slot {
        if *held == key && memo::is_fresh(*read_at, now) {
            return Ok(ClimateRead {
                record: record.clone(),
                last_good_at: None,
            });
        }
    }
    let start = climate_start(asked.at.today)
        .ok_or_else(|| TurfOpsError::InvalidData("Invalid history start".into()))?;
    match WeatherRecord::new(state.pool.clone())
        .fetch_climate_record(start, asked)
        .await
    {
        Ok(series) => {
            let record = Arc::new(ClimateRecord {
                days: series.rows,
                basis: series.basis,
            });
            *state.climate_cache.write().await = Some((key, now, record.clone()));
            Ok(ClimateRead {
                record,
                last_good_at: None,
            })
        }
        Err(e) => stand_in(Some(&key), e),
    }
}

// @spec TIME-ENG-009
/// The forecast's first `FORECAST_DAYS` full days, which the Timing page and the seasonal
/// plan's timing activities both evaluate the windows with; `None` with no forecast.
async fn forecast_days(state: &AppState) -> Result<Option<Vec<DailyForecast>>, TurfOpsError> {
    let forecast = {
        let mut service = state.sync_service.write().await;
        service.get_or_refresh().await?.forecast
    };
    // Only full days: a partial day's (high + low) / 2 is an evening or a night, not a mean.
    Ok(forecast.map(|f| f.full_days().take(FORECAST_DAYS).cloned().collect()))
}

/// The stored 5 cm soil estimate, and its daily error (°F).
async fn soil_estimate(state: &AppState) -> Result<Option<(SoilEstimate, f64)>, TurfOpsError> {
    Ok(soil_estimate_queries::all(&state.pool)
        .await?
        .into_iter()
        .find(|fit| fit.depth_cm as u32 == timing::SOIL_DEPTH_CM)
        .map(|fit| {
            (
                SoilEstimate {
                    model: fit.model,
                    calibration: fit.calibration,
                },
                fit.rmse_f,
            )
        }))
}

// @spec TIME-ENG-040
/// What 5 cm soil and daily air rest on: their spans over the current year, and the basis
/// of the newest span of each.
fn timing_basis(spans: &[BasisSpan], today: NaiveDate) -> TimingBasis {
    let jan1 = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today);
    let this_year: Vec<BasisSpan> = spans
        .iter()
        .filter(|span| span.to >= jan1)
        .map(|span| BasisSpan {
            from: span.from.max(jan1),
            ..span.clone()
        })
        .collect();
    let newest = |input: Input| {
        spans
            .iter()
            .filter(|span| span.input == input)
            .max_by_key(|span| span.to)
            .map(|span| span.basis.clone())
    };
    TimingBasis {
        soil: newest(Input::SoilTemp5),
        air: newest(Input::DailyAir),
        soil_changes: soil_story(&this_year)
            .changes
            .iter()
            .map(|change| change.on)
            .collect(),
        spans: this_year,
    }
}

/// The days the estimate filled between days one source measured.
#[derive(Debug, Clone, PartialEq)]
struct GapFills {
    measured_by: Basis,
    days: i64,
    first: NaiveDate,
    last: NaiveDate,
}

/// A lasting change of 5 cm soil basis.
#[derive(Debug, Clone, PartialEq)]
struct SoilChange {
    on: NaiveDate,
    from: Basis,
    to: Basis,
}

/// How 5 cm soil's basis went this year: the gaps the estimate filled, and the changes
/// that remain once those are set aside.
struct SoilStory {
    gap_fills: Vec<GapFills>,
    changes: Vec<SoilChange>,
}

// @spec TIME-ENG-052
/// An estimated stretch between stretches the same source measured is a gap fill; the
/// other changes of basis are lasting.
fn soil_story(spans: &[BasisSpan]) -> SoilStory {
    let mut soil: Vec<&BasisSpan> = spans
        .iter()
        .filter(|span| span.input == Input::SoilTemp5)
        .collect();
    soil.sort_by_key(|span| span.from);
    let is_fill = |i: usize| {
        i > 0
            && i + 1 < soil.len()
            && soil[i].basis.kind == BasisKind::Estimate
            && soil[i - 1].basis.kind != BasisKind::Estimate
            && soil[i - 1].basis == soil[i + 1].basis
    };
    let mut gap_fills: Vec<GapFills> = Vec::new();
    let mut lasting: Vec<&BasisSpan> = Vec::new();
    for (i, span) in soil.iter().enumerate() {
        if !is_fill(i) {
            lasting.push(span);
            continue;
        }
        let days = (span.to - span.from).num_days() + 1;
        let measured_by = &soil[i - 1].basis;
        match gap_fills.iter_mut().find(|g| g.measured_by == *measured_by) {
            Some(fills) => {
                fills.days += days;
                fills.first = fills.first.min(span.from);
                fills.last = fills.last.max(span.to);
            }
            None => gap_fills.push(GapFills {
                measured_by: measured_by.clone(),
                days,
                first: span.from,
                last: span.to,
            }),
        }
    }
    let changes = lasting
        .windows(2)
        .filter(|pair| pair[0].basis != pair[1].basis)
        .map(|pair| SoilChange {
            on: pair[1].from,
            from: pair[0].basis.clone(),
            to: pair[1].basis.clone(),
        })
        .collect();
    SoilStory { gap_fills, changes }
}

// @spec TIME-ENG-045
async fn compute(state: &AppState, profile: &LawnProfile) -> Result<TimingResponse, TurfOpsError> {
    // @spec TIME-ENG-039
    let climate = climate_read(state).await?;
    let forecast = forecast_days(state).await?;
    let estimate = soil_estimate(state).await?;
    let record = &climate.record;
    let today = state.clock.today();
    let days = &record.days;

    let profile_id = profile
        .id
        .ok_or_else(|| TurfOpsError::InvalidData("Profile missing ID".into()))?;
    // Its own range, so the feed and this page judge the same log.
    // @spec FEED-PIPE-011
    let history = queries::get_applications_for_profile_in_range(
        &state.pool,
        profile_id,
        today - Duration::days(HISTORY_LOOKBACK_DAYS),
        today + Duration::days(1),
    )
    .await?;

    let full_days: Vec<DailyForecast> = forecast.clone().unwrap_or_default();
    let daily_forecast = full_days.as_slice();
    let assessment = timing::assess(&timing::Inputs {
        today,
        days,
        forecast: daily_forecast,
        grass: profile.grass_type,
        history: &history,
        estimate: estimate.as_ref().map(|(estimate, _)| estimate),
    });

    let basis = timing_basis(&record.basis, today);
    let mut data_notes = data_notes(&NoteInputs {
        assessment: &assessment,
        profile,
        has_forecast: forecast.is_some(),
        days,
        spans: &basis.spans,
        daily_error_f: estimate.as_ref().map(|(_, error)| *error),
    });
    // @spec TIME-ENG-046
    if let Some(read_at) = climate.last_good_at {
        data_notes.insert(0, memo::last_good_note(state.clock.local(read_at)));
    }
    Ok(TimingResponse {
        generated_at: state.clock.now(),
        today,
        basis,
        context: context::build_context(days, &assessment.history_years, daily_forecast),
        series: context::build_series(&assessment.smoothed, today, &assessment.history_years),
        soil: assessment.soil,
        freeze: assessment.freeze,
        windows: assessment.windows,
        history_years: assessment.history_years,
        data_notes,
    })
}

// @spec FEED-PIPE-016, TIME-FEED-007
/// What the timing windows contribute to the feed/dashboard: one recommendation per
/// actionable window, core aeration (same window as seeding), and the fall herbicide
/// reconciled with the seeding decision. Like disease risk this is additive: a lake
/// outage leaves the feed as the rules produced it rather than failing it.
pub(crate) async fn apply_to_feed(
    state: &AppState,
    profile: &LawnProfile,
    history: &[Application],
    recommendations: &mut Vec<Recommendation>,
) {
    let response = match compute(state, profile).await {
        Ok(response) => response,
        Err(e) => {
            tracing::warn!("Timing windows unavailable for recommendations: {}", e);
            return;
        }
    };
    recommendations.extend(timing::recommendations::to_recommendations(
        &response.windows,
        &response.soil,
        response.basis.soil.as_ref(),
        profile,
        response.today,
    ));
    recommendations.extend(timing::companions::aeration(
        &response.windows,
        profile,
        history,
        response.today,
    ));
    timing::companions::reconcile_fall_herbicide(recommendations, &response.windows);
}

// @spec TIME-PLAN-006
/// Seasonal-plan activities for `year` from the timing windows. `None` (so the plan keeps
/// its own versions) when the weather record cannot answer or nothing is dated.
pub(crate) async fn plan_activities(
    state: &AppState,
    profile: &LawnProfile,
    year: i32,
) -> Option<Vec<PlannedActivity>> {
    match plan_for(state, profile, year).await {
        Ok(activities) if !activities.is_empty() => Some(activities),
        Ok(_) => None,
        Err(e) => {
            tracing::warn!("Timing windows unavailable for the seasonal plan: {}", e);
            None
        }
    }
}

async fn plan_for(
    state: &AppState,
    profile: &LawnProfile,
    year: i32,
) -> Result<Vec<PlannedActivity>, TurfOpsError> {
    // @spec TIME-PLAN-007
    // The same inputs as the Timing page, forecast and estimate included, so the plan's
    // status matches the window's state there.
    // @spec TIME-ENG-039
    let record = climate_record(state).await?;
    let forecast = forecast_days(state).await?;
    let estimate = soil_estimate(state).await?;
    let today = state.clock.today();
    let days = &record.days;

    // The log spans that decide done/blocked reach from the previous mid-November
    // (dormant seeding) into the following February.
    let profile_id = profile
        .id
        .ok_or_else(|| TurfOpsError::InvalidData("Profile missing ID".into()))?;
    let invalid_year = || TurfOpsError::InvalidData(format!("Invalid year: {year}"));
    let applications = queries::get_applications_for_profile_in_range(
        &state.pool,
        profile_id,
        NaiveDate::from_ymd_opt(year - 1, 11, 1).ok_or_else(invalid_year)?,
        NaiveDate::from_ymd_opt(year + 1, 3, 1).ok_or_else(invalid_year)?,
    )
    .await?;

    let data = timing::SeasonData::build_with(
        days,
        forecast.as_deref().unwrap_or_default(),
        today.year(),
        estimate.as_ref().map(|(estimate, _)| estimate),
    );
    let season = data.season(days, today);
    Ok(timing::plan::activities(
        &season,
        profile.grass_type,
        &applications,
        year,
    ))
}

/// What the data notes are written from.
struct NoteInputs<'a> {
    assessment: &'a Assessment,
    profile: &'a LawnProfile,
    has_forecast: bool,
    days: &'a [crate::models::climate::ClimateDay],
    /// The current year's basis spans.
    spans: &'a [BasisSpan],
    /// The stored 5 cm estimate's daily error (°F), when there is one.
    daily_error_f: Option<f64>,
}

// @spec TIME-ENG-044, TIME-ENG-004, TIME-ENG-050, TIME-ENG-052
/// Plain-language caveats about the inputs behind this response.
fn data_notes(inputs: &NoteInputs) -> Vec<String> {
    let NoteInputs {
        assessment,
        profile,
        ..
    } = inputs;
    let mut notes = Vec::new();
    if !assessment.data_fresh {
        notes.push(match assessment.soil.as_of {
            Some(date) => format!(
                "Soil data ends {date}. Until it catches up, windows follow their typical \
                 dates instead of this season's soil."
            ),
            None => "No soil data is available; windows follow their typical dates.".into(),
        });
    }
    // The soil in use now, or a date this season, rests on the estimate.
    let estimated_now = inputs
        .days
        .iter()
        .rev()
        .find(|d| d.soil_temp_5_f.is_some())
        .is_some_and(|d| d.soil_estimated);
    let estimated_date = assessment.windows.iter().any(|w| {
        [
            Some(&w.opens),
            w.ideal_from.as_ref(),
            w.ideal_until.as_ref(),
            Some(&w.closes),
        ]
        .into_iter()
        .flatten()
        .any(|b| b.estimated)
    });
    if estimated_now || estimated_date {
        notes.push(match inputs.daily_error_f {
            Some(error) => format!(
                "5 cm soil temperature this season is estimated for your location from air \
                 temperature, typically within {error:.1}°F of a measured day; dates from it \
                 are marked estimated."
            ),
            None => "5 cm soil temperature this season is estimated for your location from \
                     air temperature; dates from it are marked estimated."
                .into(),
        });
    }
    let story = soil_story(inputs.spans);
    for fills in &story.gap_fills {
        let when = if fills.first == fills.last {
            format!("on {}", fills.first.format("%b %-d"))
        } else {
            format!(
                "on {} day{} this year ({} – {})",
                fills.days,
                if fills.days == 1 { "" } else { "s" },
                fills.first.format("%b %-d"),
                fills.last.format("%b %-d")
            )
        };
        notes.push(format!(
            "The 5 cm soil from {} had gaps {when}; the soil estimate for your location filled \
             them.",
            fills.measured_by.source
        ));
    }
    for change in &story.changes {
        notes.push(format!(
            "From {}, 5 cm soil comes from {} instead of {}.",
            change.on.format("%b %-d"),
            soil_words(&change.to),
            soil_words(&change.from)
        ));
    }
    for name in &assessment.withheld {
        notes.push(format!(
            "{name} is not shown: soil temperature for your location is estimated, and the \
             estimate cannot date this window reliably."
        ));
    }
    let years = assessment.history_years.len();
    if years < MIN_SOLID_HISTORY_YEARS {
        notes.push(format!(
            "Only {years} complete year(s) of soil history — typical dates are rough."
        ));
    }
    if !inputs.has_forecast {
        notes.push("Forecast unavailable — no soil outlook or weather alerts.".into());
    }
    if !profile.grass_type.is_cool_season() {
        notes.push("Seeding windows cover cool-season turf only.".into());
    }
    notes
}

/// A soil basis in a sentence: the estimate is the soil estimate, not the hourly one.
fn soil_words(basis: &Basis) -> String {
    match basis.kind {
        BasisKind::Estimate => "the soil estimate for your location".into(),
        _ => basis.in_words(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::climate::ClimateDay;
    use crate::models::timing::{
        BoundaryView, DateSource, FreezeDates, SoilNow, TimingWindow, WindowId, WindowPriority,
        WindowState,
    };

    // @spec TIME-ENG-039
    #[test]
    fn the_climate_record_starts_from_the_lawns_year() {
        let new_years_eve = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
        assert_eq!(
            climate_start(new_years_eve),
            NaiveDate::from_ymd_opt(2025 - timing::HISTORY_YEARS, 1, 1)
        );
    }

    fn assessment(data_fresh: bool, years: usize) -> Assessment {
        Assessment {
            windows: Vec::new(),
            soil: SoilNow {
                avg_5day_f: Some(66.0),
                as_of: NaiveDate::from_ymd_opt(2026, 9, 10),
                depth_cm: 5,
            },
            freeze: FreezeDates {
                last_spring: None,
                first_fall: None,
                last_spring_this_year: None,
                first_fall_this_year: None,
            },
            history_years: (0..years as i32).map(|i| 2015 + i).collect(),
            smoothed: Vec::new(),
            data_fresh,
            withheld: Vec::new(),
        }
    }

    fn date(month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, month, day).unwrap()
    }

    fn soil_day(on: NaiveDate, estimated: bool) -> ClimateDay {
        ClimateDay {
            date: on,
            soil_temp_5_f: Some(60.0),
            soil_estimated: estimated,
            air_min_f: None,
            air_avg_f: None,
            gdd50: None,
        }
    }

    fn station() -> Basis {
        Basis {
            kind: BasisKind::Station,
            source: "Avondale 2 N".into(),
            distance_miles: Some(20.0),
            approximate: false,
        }
    }

    fn estimate() -> Basis {
        Basis {
            kind: BasisKind::Estimate,
            source: "Soil estimate".into(),
            distance_miles: None,
            approximate: false,
        }
    }

    fn span(input: Input, from: NaiveDate, to: NaiveDate, basis: Basis) -> BasisSpan {
        BasisSpan {
            input,
            from,
            to,
            basis,
        }
    }

    /// The notes for `assessment`, on Sep 20 with a healthy record otherwise.
    fn notes_for(
        assessment: &Assessment,
        profile: &LawnProfile,
        has_forecast: bool,
        days: &[ClimateDay],
        spans: &[BasisSpan],
    ) -> Vec<String> {
        data_notes(&NoteInputs {
            assessment,
            profile,
            has_forecast,
            days,
            spans,
            daily_error_f: Some(2.14),
        })
    }

    fn healthy(assessment: &Assessment, profile: &LawnProfile) -> Vec<String> {
        notes_for(assessment, profile, true, &[], &[])
    }

    // @spec TIME-ENG-044
    #[test]
    fn no_notes_when_inputs_are_healthy() {
        let notes = healthy(&assessment(true, 10), &LawnProfile::default());
        assert!(notes.is_empty(), "{notes:?}");
    }

    // @spec TIME-ENG-004
    #[test]
    fn a_warm_season_lawn_is_told_seeding_windows_are_cool_season_only() {
        let warm = LawnProfile {
            grass_type: crate::models::GrassType::Bermuda,
            ..LawnProfile::default()
        };
        assert_eq!(
            healthy(&assessment(true, 10), &warm),
            vec!["Seeding windows cover cool-season turf only.".to_string()]
        );
        let mixed = LawnProfile {
            grass_type: crate::models::GrassType::Mixed,
            ..LawnProfile::default()
        };
        assert!(healthy(&assessment(true, 10), &mixed).is_empty());
    }

    // @spec TIME-ENG-044
    #[test]
    fn notes_flag_stale_soil_thin_history_and_missing_forecast() {
        let notes = notes_for(
            &assessment(false, 2),
            &LawnProfile::default(),
            false,
            &[],
            &[],
        );
        assert_eq!(
            notes,
            vec![
                "Soil data ends 2026-09-10. Until it catches up, windows follow their typical \
                 dates instead of this season's soil.",
                "Only 2 complete year(s) of soil history — typical dates are rough.",
                "Forecast unavailable — no soil outlook or weather alerts.",
            ]
        );
    }

    // @spec TIME-ENG-052
    #[test]
    fn estimated_soil_is_noted_only_when_in_use_now_or_dating_this_season() {
        let healthy = |days: &[ClimateDay], assessment: &Assessment| {
            notes_for(assessment, &LawnProfile::default(), true, days, &[])
        };
        // A day this spring the estimate filled, the soil in use now measured: no note.
        let filled_in_spring = [soil_day(date(4, 20), true), soil_day(date(9, 19), false)];
        assert!(healthy(&filled_in_spring, &assessment(true, 10)).is_empty());

        let estimated_note = "5 cm soil temperature this season is estimated for your \
                              location from air temperature, typically within 2.1°F of a \
                              measured day; dates from it are marked estimated.";
        let in_use_now = [soil_day(date(9, 18), false), soil_day(date(9, 19), true)];
        assert_eq!(
            healthy(&in_use_now, &assessment(true, 10)),
            vec![estimated_note]
        );

        let mut dated = assessment(true, 10);
        let boundary = |estimated: bool| BoundaryView {
            label: "5 cm soil cools to 70°F".into(),
            typical: None,
            date: Some(date(9, 14)),
            source: DateSource::Observed,
            passed: true,
            days_held: None,
            estimated,
            typical_error_days: estimated.then_some(2.0),
        };
        dated.windows = vec![TimingWindow {
            id: WindowId::FallPreEmergent,
            name: "Fall Pre-Emergent".into(),
            priority: WindowPriority::Primary,
            season_year: 2026,
            state: WindowState::Ideal,
            headline: String::new(),
            detail: String::new(),
            opens: boundary(true),
            ideal_from: None,
            ideal_until: None,
            closes: boundary(false),
            done_on: None,
            conflict: None,
            guidance: Vec::new(),
        }];
        assert_eq!(healthy(&filled_in_spring, &dated), vec![estimated_note]);
    }

    // @spec TIME-ENG-052
    #[test]
    fn gaps_the_estimate_filled_are_one_note_and_only_lasting_changes_are_dated() {
        let spans = [
            span(Input::SoilTemp5, date(1, 1), date(2, 21), station()),
            span(Input::SoilTemp5, date(2, 22), date(2, 22), estimate()),
            span(Input::DailyAir, date(1, 1), date(9, 19), estimate()),
            span(Input::SoilTemp5, date(2, 23), date(5, 11), station()),
            span(Input::SoilTemp5, date(5, 12), date(5, 15), estimate()),
            span(Input::SoilTemp5, date(5, 16), date(8, 31), station()),
            span(Input::SoilTemp5, date(9, 1), date(9, 19), estimate()),
        ];
        assert_eq!(
            notes_for(
                &assessment(true, 10),
                &LawnProfile::default(),
                true,
                &[],
                &spans
            ),
            vec![
                "The 5 cm soil from Avondale 2 N had gaps on 5 days this year (Feb 22 – \
                 May 15); the soil estimate for your location filled them.",
                "From Sep 1, 5 cm soil comes from the soil estimate for your location \
                 instead of Avondale 2 N (20 mi).",
            ]
        );
        let basis = timing_basis(&spans, date(9, 20));
        assert_eq!(basis.soil_changes, vec![date(9, 1)]);

        let one_day = [
            span(Input::SoilTemp5, date(1, 1), date(4, 19), station()),
            span(Input::SoilTemp5, date(4, 20), date(4, 20), estimate()),
            span(Input::SoilTemp5, date(4, 21), date(9, 19), station()),
        ];
        assert_eq!(
            notes_for(
                &assessment(true, 10),
                &LawnProfile::default(),
                true,
                &[],
                &one_day
            ),
            vec![
                "The 5 cm soil from Avondale 2 N had gaps on Apr 20; the soil estimate for \
                 your location filled them."
            ]
        );
        assert!(timing_basis(&one_day, date(9, 20)).soil_changes.is_empty());
    }

    // @spec TIME-ENG-050
    #[test]
    fn a_withheld_window_is_named() {
        let mut withheld = assessment(true, 10);
        withheld.withheld = vec!["Spring pre-emergent".into()];
        assert_eq!(
            healthy(&withheld, &LawnProfile::default()),
            vec![
                "Spring pre-emergent is not shown: soil temperature for your location is \
                 estimated, and the estimate cannot date this window reliably."
            ]
        );
    }

    // @spec TIME-ENG-040
    #[test]
    fn the_basis_is_this_years_spans_and_each_inputs_newest() {
        let spans = [
            span(
                Input::SoilTemp5,
                date(1, 1) - Duration::days(400),
                date(6, 3),
                station(),
            ),
            span(Input::SoilTemp5, date(6, 4), date(9, 19), estimate()),
            span(
                Input::DailyAir,
                date(1, 1) - Duration::days(400),
                date(9, 19),
                estimate(),
            ),
        ];
        let basis = timing_basis(&spans, date(9, 20));
        assert_eq!(basis.soil, Some(estimate()));
        assert_eq!(basis.air, Some(estimate()));
        let froms: Vec<NaiveDate> = basis.spans.iter().map(|s| s.from).collect();
        assert_eq!(froms, vec![date(1, 1), date(6, 4), date(1, 1)]);
    }
}
