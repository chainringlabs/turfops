//! The record's daily contracts: series of the lawn's days, each with its basis.

use super::record::{Asked, View, WeatherRecord};
use super::resolve::{self, DayAir};
use crate::datasources::weather::{ClimateDay, DailyGddRow};
use crate::db::weather_queries::fetch_state;
use crate::error::{Result, TurfOpsError};
use crate::models::seasonal_plan::DailySoilTempAvg;
use crate::models::weather_basis::{Basis, BasisSpan, Figure, Series};
use crate::models::weather_record::{Input, SourceKind};
use chrono::{Datelike, Duration, NaiveDate};

/// The base of growing degree days, °F.
const GDD_BASE_F: f64 = 50.0;

/// The inputs whose whole history a long contract reads.
const LONG_HISTORY: [Input; 3] = [Input::SoilTemp5, Input::SoilTemp10, Input::DailyAir];

// @spec WX-REC-API-008
fn gdd50(air: &DayAir) -> f64 {
    ((air.max_f + air.min_f) / 2.0 - GDD_BASE_F).max(0.0)
}

/// The last complete local day.
fn yesterday(asked: &Asked) -> NaiveDate {
    asked.at.today - Duration::days(1)
}

impl WeatherRecord {
    /// The resolved 10 cm soil temperature of each complete day in the span that has one.
    // @spec WX-REC-API-005, WX-REC-FAIL-002
    pub async fn fetch_daily_soil_temp_averages(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        asked: Asked,
    ) -> Result<Series<DailySoilTempAvg>> {
        let view = self.view(asked).await?;
        view.history_held().await?;
        let soil = view
            .resolve(Input::SoilTemp10, start, end.min(yesterday(&asked)))
            .await?;
        Ok(Series {
            rows: soil
                .days
                .iter()
                .map(|(date, value)| DailySoilTempAvg {
                    date: *date,
                    avg_temp_f: value.value,
                })
                .collect(),
            basis: resolve::spans(Input::SoilTemp10, &soil.days, &soil.candidates),
        })
    }

    /// `(day, air mean, 10 cm soil)` for each complete day that has both.
    // @spec WX-REC-API-006, WX-REC-FAIL-002
    pub async fn fetch_daily_paired_averages(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        asked: Asked,
    ) -> Result<Series<(NaiveDate, f64, f64)>> {
        let view = self.view(asked).await?;
        view.history_held().await?;
        let end = end.min(yesterday(&asked));
        let mut air = view.resolve_air(start, end).await?;
        let mut soil = view.resolve(Input::SoilTemp10, start, end).await?;
        air.days.retain(|day, _| soil.days.contains_key(day));
        soil.days.retain(|day, _| air.days.contains_key(day));
        let rows = air
            .days
            .iter()
            .map(|(day, air)| (*day, air.value.mean_f, soil.days[day].value))
            .collect();
        let mut basis = resolve::spans(Input::DailyAir, &air.days, &air.candidates);
        basis.extend(resolve::spans(
            Input::SoilTemp10,
            &soil.days,
            &soil.candidates,
        ));
        Ok(Series { rows, basis })
    }

    /// `(day, maximum, minimum, gdd50)` for each day with a resolved daily air.
    // @spec WX-REC-API-008, WX-REC-FAIL-003
    pub async fn fetch_daily_gdd(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        asked: Asked,
    ) -> Result<Series<DailyGddRow>> {
        let view = self.view(asked).await?;
        view.daily_gdd(start, end).await
    }

    /// Growing degree days from January 1 of `today`'s year through `today`; nothing
    /// when no day in that span has a value.
    // @spec WX-REC-API-009, WX-REC-BASIS-004, WX-REC-FAIL-003
    pub async fn fetch_gdd_ytd(&self, asked: Asked) -> Result<Option<Figure<f64>>> {
        let today = asked.at.today;
        let jan1 = NaiveDate::from_ymd_opt(today.year(), 1, 1)
            .ok_or_else(|| TurfOpsError::Internal(format!("no January 1 in {}", today.year())))?;
        let series = self.view(asked).await?.daily_gdd(jan1, today).await?;
        Ok(figure(&series.basis).map(|(basis, mixed)| Figure {
            value: series.rows.iter().map(|(_, _, _, gdd)| gdd).sum(),
            basis,
            mixed,
        }))
    }

    /// One row per local day from `start` that has any of: 5 cm soil, daily air.
    // @spec WX-REC-API-011, WX-REC-FAIL-002
    pub async fn fetch_climate_record(
        &self,
        start: NaiveDate,
        asked: Asked,
    ) -> Result<Series<ClimateDay>> {
        let view = self.view(asked).await?;
        view.history_held().await?;
        let end = yesterday(&asked);
        let air = view.resolve_air(start, end).await?;
        let soil = view.resolve(Input::SoilTemp5, start, end).await?;
        let mut days: Vec<NaiveDate> = air.days.keys().chain(soil.days.keys()).copied().collect();
        days.sort_unstable();
        days.dedup();
        let rows = days
            .into_iter()
            .map(|date| {
                let air = air.days.get(&date).map(|r| &r.value);
                ClimateDay {
                    date,
                    soil_temp_5_f: soil.days.get(&date).map(|r| r.value),
                    air_min_f: air.map(|a| a.min_f),
                    air_avg_f: air.map(|a| a.mean_f),
                    gdd50: air.map(gdd50),
                }
            })
            .collect();
        let mut basis = resolve::spans(Input::SoilTemp5, &soil.days, &soil.candidates);
        basis.extend(resolve::spans(Input::DailyAir, &air.days, &air.candidates));
        Ok(Series { rows, basis })
    }
}

/// The basis of a figure over a period: its newest day's, and whether there were others.
// @spec WX-REC-BASIS-004
fn figure(spans: &[BasisSpan]) -> Option<(Basis, bool)> {
    let newest = spans.iter().max_by_key(|span| span.to)?;
    let mixed = spans.iter().any(|span| span.basis != newest.basis);
    Some((newest.basis.clone(), mixed))
}

impl View<'_> {
    async fn daily_gdd(&self, start: NaiveDate, end: NaiveDate) -> Result<Series<DailyGddRow>> {
        let air = self.resolve_air(start, end).await?;
        Ok(Series {
            rows: air
                .days
                .iter()
                .map(|(day, r)| (*day, r.value.max_f, r.value.min_f, gdd50(&r.value)))
                .collect(),
            basis: resolve::spans(Input::DailyAir, &air.days, &air.candidates),
        })
    }

    /// Passes once every source the long-history inputs rest on holds the span it is
    /// wanted for; until then, the history-still-loading outage with the years in hand.
    // @spec WX-REC-FAIL-002
    async fn history_held(&self) -> Result<()> {
        let states = fetch_state::all(&self.record.pool).await?;
        let this_year = self.asked.at.today.year();
        let (mut held, mut wanted) = (u32::MAX, 0u32);
        for input in LONG_HISTORY {
            // A sensor has no past to download.
            for source in self.sources(input).filter(|s| s.kind != SourceKind::Sensor) {
                let products: Vec<_> = states.iter().filter(|s| s.source_id == source.id).collect();
                if products.is_empty() {
                    (held, wanted) = (0, wanted.max(1));
                }
                for state in products {
                    let years_from = |day: NaiveDate| (this_year - day.year() + 1).max(0) as u32;
                    let in_hand = match state.held {
                        Some((from, _)) if from <= state.wanted_from => {
                            years_from(state.wanted_from)
                        }
                        // A year held only in part is not yet in hand.
                        Some((from, _)) => years_from(from) - u32::from(from.ordinal() > 1),
                        None => 0,
                    };
                    held = held.min(in_hand);
                    wanted = wanted.max(years_from(state.wanted_from));
                }
            }
        }
        if wanted == 0 || held >= wanted {
            return Ok(());
        }
        Err(TurfOpsError::history_loading(held, wanted))
    }
}
