//! Weather history as daily rows. The REST `/historical` endpoint downsamples hourly
//! points for its charts, so it cannot give a true daily rain total; this tool joins two
//! daily reads of the weather record instead: hourly air, humidity and rain per local day,
//! and the memoized climate record (5 cm soil, daily air, `gdd50`).

use super::{json_result, tool_error};
use crate::api;
use crate::error::TurfOpsError;
use crate::logic::data_sync::record_asked;
use crate::logic::weather_record::record::WeatherRecord;
use crate::mcp::TurfOpsMcp;
use crate::models::climate::ClimateRecord;
use crate::models::weather_basis::{Basis, BasisKind, BasisSpan};
use crate::models::weather_record::Input;
use crate::models::{celsius_to_fahrenheit, DailyWeather, AFTERNOON_HOURS, DAWN_HOURS};
use chrono::{Duration, NaiveDate};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{tool, tool_router, ErrorData};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const DEFAULT_DAYS: i64 = 30;
const MAX_DAYS: i64 = 120;
const MM_PER_INCH: f64 = 25.4;

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct WeatherHistoryParams {
    /// Days back from today (default 30, max 120).
    pub days: Option<i64>,
}

/// One lawn-local day. Any value can be missing (a source not yet reporting).
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct HistoryDay {
    pub date: NaiveDate,
    pub air_mean_f: Option<f64>,
    pub air_min_f: Option<f64>,
    pub air_max_f: Option<f64>,
    pub rh_mean_pct: Option<f64>,
    pub rain_in: Option<f64>,
    pub soil_5cm_f: Option<f64>,
    pub gdd50: Option<f64>,
    /// Hourly observations behind the air / humidity / rain values (24 = complete day).
    pub hours_observed: Option<f64>,
    /// What the day's 5 cm soil rests on: `measured`, `sensor`, `estimated` or `approximate`.
    pub soil_basis: Option<&'static str>,
    /// What the day's air rests on, in the same words.
    pub air_basis: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct WeatherHistory {
    pub start: NaiveDate,
    pub days: Vec<HistoryDay>,
    /// The sources behind the rows' basis words, each named once under its word.
    pub basis: BTreeMap<&'static str, Vec<String>>,
    pub rain_total_in: f64,
    pub gdd50_total: f64,
    pub note: &'static str,
}

#[tool_router(router = weather_tools, vis = "pub(in crate::mcp)")]
impl TurfOpsMcp {
    // @spec MCP-HIST-001
    #[tool(
        description = "Daily weather for the last N days (default 30, max 120): air mean / \
                       min / max °F, mean humidity, rain (inches), 5 cm soil mean °F, GDD \
                       base 50 and hours observed, each day's soil and air basis (measured, \
                       sensor, estimated or approximate) with the sources behind them, plus \
                       rain and GDD totals. Use for 'how much rain have we had', 'has it \
                       been dry', recent soil trends. Today is partial.",
        annotations(read_only_hint = true)
    )]
    async fn weather_history(
        &self,
        Parameters(params): Parameters<WeatherHistoryParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let days = params.days.unwrap_or(DEFAULT_DAYS).clamp(1, MAX_DAYS);
        match self.history(days).await {
            Ok(history) => json_result(&history),
            Err(err) => Ok(tool_error(err)),
        }
    }

    // @spec MCP-TOOL-028
    #[tool(
        description = "Where each weather value comes from: per input (soil temperature and \
                       moisture by depth, air, humidity, rain, daily air) the basis in use \
                       (a named station and its distance, the owner's sensor, or an estimate \
                       for the location), the sources ranked behind it, the newest value and, \
                       for an estimated input, the estimate's typical error and which \
                       crossings it may date; per source whether its fetches succeed, the \
                       history held and its credit; the forecast provider and its status. \
                       Use for 'where does this number come from', 'why is it estimated', \
                       'is a source down'.",
        annotations(read_only_hint = true)
    )]
    async fn sources(&self) -> Result<CallToolResult, ErrorData> {
        super::call_trimmed(
            api::sources::read::get_sources(axum::extract::State(self.state.clone())),
            crate::mcp::trim::sources,
        )
        .await
    }
}

impl TurfOpsMcp {
    // @spec MCP-HIST-003, MCP-HIST-007
    async fn history(&self, days: i64) -> Result<WeatherHistory, TurfOpsError> {
        let asked = record_asked(&self.state.pool, &self.state.clock).await?;
        let start = history_start(asked.at.today, days);
        let record = WeatherRecord::new(self.state.pool.clone());
        let (observed, climate) = tokio::try_join!(
            record.fetch_daily_disease_inputs(start, DAWN_HOURS, AFTERNOON_HOURS, asked),
            api::timing::climate_record(&self.state),
        )?;
        Ok(join_days(&observed.days, &climate, start))
    }
}

/// First day of a `days`-long history window ending on the lawn's `today`.
// @spec MCP-HIST-002
fn history_start(today: NaiveDate, days: i64) -> NaiveDate {
    today - Duration::days(days)
}

/// A basis in the history's words.
fn basis_word(basis: &Basis) -> &'static str {
    match basis.kind {
        BasisKind::Station => "measured",
        BasisKind::Sensor => "sensor",
        BasisKind::Estimate if basis.approximate => "approximate",
        BasisKind::Estimate => "estimated",
    }
}

/// A source's name as the history gives it, a station with its distance.
fn source_name(basis: &Basis) -> String {
    match basis.distance_miles {
        Some(miles) => format!("{} ({miles:.0} mi)", basis.source),
        None => basis.source.clone(),
    }
}

/// The basis of `input` on `date` among the record's spans.
fn basis_on(spans: &[BasisSpan], input: Input, date: NaiveDate) -> Option<&Basis> {
    spans
        .iter()
        .find(|s| s.input == input && s.from <= date && date <= s.to)
        .map(|s| &s.basis)
}

/// Merge the hourly per-day aggregates with the climate record, one row per date from
/// `start`, ascending, each with the basis of its soil and air.
// @spec MCP-HIST-003, MCP-HIST-004, MCP-HIST-005, MCP-HIST-006, MCP-HIST-008
fn join_days(
    observed: &[DailyWeather],
    climate: &ClimateRecord,
    start: NaiveDate,
) -> WeatherHistory {
    let mut rows: BTreeMap<NaiveDate, HistoryDay> = BTreeMap::new();
    let mut named: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut note = |basis: &Basis| {
        let word = basis_word(basis);
        let names = named.entry(word).or_default();
        let name = source_name(basis);
        if !names.contains(&name) {
            names.push(name);
        }
        word
    };
    for day in observed.iter().filter(|d| d.date >= start) {
        let air_basis = day.basis.as_ref().map(&mut note);
        let r = entry(&mut rows, day.date);
        r.air_basis = air_basis;
        r.air_mean_f = Some(round1(celsius_to_fahrenheit(day.temp_mean_c)));
        r.air_min_f = Some(round1(celsius_to_fahrenheit(day.temp_min_c)));
        r.air_max_f = Some(round1(celsius_to_fahrenheit(day.temp_max_c)));
        r.rh_mean_pct = Some(round1(day.rh_mean));
        r.rain_in = Some(round2(day.precip_mm / MM_PER_INCH));
        r.hours_observed = Some(day.hours_covered);
    }
    for day in climate.days.iter().filter(|d| d.date >= start) {
        let soil_basis = day
            .soil_temp_5_f
            .and(basis_on(&climate.basis, Input::SoilTemp5, day.date))
            .map(&mut note);
        let fills_air =
            !rows.get(&day.date).is_some_and(|r| r.air_mean_f.is_some()) && day.air_avg_f.is_some();
        let daily_air_basis = basis_on(&climate.basis, Input::DailyAir, day.date)
            .filter(|_| fills_air)
            .map(&mut note);
        let r = entry(&mut rows, day.date);
        r.soil_5cm_f = day.soil_temp_5_f.map(round1);
        r.soil_basis = soil_basis;
        r.gdd50 = day.gdd50.map(round1);
        // The daily air fills a day the hourly air has nothing for.
        if fills_air {
            r.air_mean_f = day.air_avg_f.map(round1);
            r.air_min_f = r.air_min_f.or(day.air_min_f.map(round1));
            r.air_basis = daily_air_basis;
        }
    }
    let days: Vec<HistoryDay> = rows.into_values().collect();
    WeatherHistory {
        start,
        basis: named,
        rain_total_in: round2(days.iter().filter_map(|d| d.rain_in).sum()),
        gdd50_total: round1(days.iter().filter_map(|d| d.gdd50).sum()),
        days,
        note: "Lawn-local days from the weather record. A day with fewer than 24 hours \
               observed is partial; its totals are low and its min / max may be missing the \
               dawn low or afternoon high.",
    }
}

fn entry(rows: &mut BTreeMap<NaiveDate, HistoryDay>, date: NaiveDate) -> &mut HistoryDay {
    rows.entry(date).or_insert_with(|| HistoryDay {
        date,
        ..HistoryDay::default()
    })
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::climate::ClimateDay;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    // @spec MCP-HIST-002
    #[test]
    fn the_history_window_counts_back_from_the_lawns_today() {
        assert_eq!(history_start(d("2025-12-31"), 30), d("2025-12-01"));
    }

    fn observed(date: &str, mean_c: f64, precip_mm: f64, hours: f64) -> DailyWeather {
        DailyWeather {
            date: d(date),
            temp_mean_c: mean_c,
            temp_min_c: mean_c - 5.0,
            temp_max_c: mean_c + 5.0,
            rh_mean: 72.44,
            hours_rh90: 0.0,
            leaf_wetness_hours: 0.0,
            precip_mm,
            dew_point_max_c: 10.0,
            hours_covered: hours,
            is_forecast: false,
            covers_dawn: true,
            covers_afternoon: true,
            basis: None,
        }
    }

    fn climate(date: &str, soil: Option<f64>, gdd: Option<f64>) -> ClimateDay {
        ClimateDay {
            date: d(date),
            soil_temp_5_f: soil,
            soil_estimated: false,
            air_min_f: Some(50.0),
            air_avg_f: Some(60.0),
            gdd50: gdd,
        }
    }

    fn record(days: Vec<ClimateDay>) -> ClimateRecord {
        ClimateRecord {
            days,
            basis: Vec::new(),
        }
    }

    fn basis(kind: BasisKind, source: &str, miles: Option<f64>, approximate: bool) -> Basis {
        Basis {
            kind,
            source: source.into(),
            distance_miles: miles,
            approximate,
        }
    }

    fn span(input: Input, from: &str, to: &str, basis: Basis) -> BasisSpan {
        BasisSpan {
            input,
            from: d(from),
            to: d(to),
            basis,
        }
    }

    // @spec MCP-HIST-004, MCP-HIST-008
    #[test]
    fn each_day_says_what_its_soil_and_air_rest_on_and_names_the_sources_once() {
        let station = basis(BasisKind::Station, "Avondale 2 N", Some(0.6), false);
        let estimate = basis(
            BasisKind::Estimate,
            "Soil temperature estimate",
            None,
            false,
        );
        let analysis = basis(BasisKind::Estimate, "Hourly analysis", None, true);
        let grid = basis(BasisKind::Estimate, "PRISM daily grid", None, false);
        let mut hourly = observed("2026-09-20", 20.0, 0.0, 24.0);
        hourly.basis = Some(analysis);
        let mut climate = record(vec![
            climate("2026-09-19", Some(66.0), Some(4.0)),
            climate("2026-09-20", Some(65.0), Some(5.0)),
        ]);
        climate.basis = vec![
            span(Input::SoilTemp5, "2026-09-19", "2026-09-19", estimate),
            span(Input::SoilTemp5, "2026-09-20", "2026-09-20", station),
            span(Input::DailyAir, "2026-09-19", "2026-09-20", grid),
        ];
        let history = join_days(&[hourly], &climate, d("2026-09-19"));
        let words: Vec<(Option<&str>, Option<&str>)> = history
            .days
            .iter()
            .map(|day| (day.soil_basis, day.air_basis))
            .collect();
        assert_eq!(
            words,
            vec![
                (Some("estimated"), Some("estimated")),
                (Some("measured"), Some("approximate")),
            ]
        );
        let named: Vec<(&str, Vec<String>)> = history
            .basis
            .iter()
            .map(|(word, names)| (*word, names.clone()))
            .collect();
        assert_eq!(
            named,
            vec![
                ("approximate", vec!["Hourly analysis".to_string()]),
                (
                    "estimated",
                    vec![
                        "Soil temperature estimate".to_string(),
                        "PRISM daily grid".to_string()
                    ]
                ),
                ("measured", vec!["Avondale 2 N (1 mi)".to_string()]),
            ]
        );
    }

    // @spec MCP-HIST-004, MCP-HIST-006
    #[test]
    fn joins_by_date_converts_units_and_totals() {
        let history = join_days(
            &[
                observed("2026-09-20", 20.0, 25.4, 24.0),
                observed("2026-09-21", 10.0, 12.7, 9.0),
            ],
            &record(vec![
                climate("2026-09-20", Some(68.26), Some(18.0)),
                climate("2026-09-21", Some(66.0), Some(4.5)),
            ]),
            d("2026-09-20"),
        );
        assert_eq!(history.days.len(), 2);
        let first = &history.days[0];
        assert_eq!(first.date, d("2026-09-20"));
        assert_eq!(first.air_mean_f, Some(68.0));
        assert_eq!(first.air_min_f, Some(59.0));
        assert_eq!(first.air_max_f, Some(77.0));
        assert_eq!(first.rh_mean_pct, Some(72.4));
        assert_eq!(first.rain_in, Some(1.0));
        assert_eq!(first.soil_5cm_f, Some(68.3));
        assert_eq!(history.days[1].hours_observed, Some(9.0));
        assert_eq!(history.rain_total_in, 1.5);
        assert_eq!(history.gdd50_total, 22.5);
    }

    // @spec MCP-HIST-003, MCP-HIST-005
    #[test]
    fn days_before_start_are_dropped_and_daily_air_fills_missing_air() {
        let history = join_days(
            &[observed("2026-09-10", 20.0, 0.0, 24.0)],
            &record(vec![
                climate("2026-09-10", None, None),
                climate("2026-09-21", None, Some(3.0)),
            ]),
            d("2026-09-15"),
        );
        assert_eq!(history.days.len(), 1);
        let only = &history.days[0];
        assert_eq!(only.date, d("2026-09-21"));
        assert_eq!(only.air_mean_f, Some(60.0));
        assert_eq!(only.air_min_f, Some(50.0));
        assert_eq!(only.air_max_f, None);
        assert_eq!(only.rain_in, None);
    }
}
