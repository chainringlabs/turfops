//! What the record wants fetched: the present first, then the history it does not hold
//! yet, newest year first.

#[cfg(test)]
#[path = "due_tests.rs"]
mod tests;

use crate::datasources::cadence::Product;
use crate::logic::soil_estimate::calibration::train_from;
use crate::models::weather_record::{FetchState, Source, SourceKind, SourceRole};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc};

/// Years of history kept behind the current year: what the typical dates read.
pub const HISTORY_YEARS: i32 = 15;
/// How far back the hourly analysis is kept from: a day past the 92 its source serves.
pub const ANALYSIS_DAYS: i64 = 93;
/// With nothing held, a first fetch of a daily grid reaches back this far.
const FIRST_GRID_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// The newest days: new data may exist, and recent values may have been revised.
    Current,
    /// Days wanted and not yet held.
    Backfill,
}

/// One fetch the record wants.
#[derive(Debug, Clone, PartialEq)]
pub struct Fetch {
    pub source: Source,
    pub product: Product,
    /// First and last day wanted, both included.
    pub span: (NaiveDate, NaiveDate),
    pub purpose: Purpose,
}

/// What is fetched for a source in the plan. A sensor is read through its mapping and
/// the soil estimate is computed, so neither is fetched here.
pub fn product_of(source: &Source) -> Option<Product> {
    match source.kind {
        // A station that only calibrates the soil estimate is read as daily values.
        SourceKind::Station if source.role == SourceRole::Calibration => {
            Some(Product::CalibrationDays)
        }
        SourceKind::Station => Some(Product::StationHours),
        SourceKind::Grid if source.key.contains("grid:nrcc") => Some(Product::NrccDays),
        SourceKind::Grid => Some(Product::PrismDays),
        SourceKind::Analysis => Some(Product::AnalysisHours),
        SourceKind::Sensor | SourceKind::Estimate => None,
    }
}

/// The first day a product is wanted from. `located_on` is when the source came into
/// use.
// @spec WX-REC-STORE-008
pub fn wanted_from(product: Product, today: NaiveDate, located_on: NaiveDate) -> NaiveDate {
    match product {
        Product::AnalysisHours => located_on - Duration::days(ANALYSIS_DAYS),
        // The fallback grid has no history of its own to hold.
        Product::NrccDays => today,
        _ => NaiveDate::from_ymd_opt(today.year() - HISTORY_YEARS, 1, 1).unwrap_or(today),
    }
}

/// What a source is fetched as and from when, or nothing for a source that is not
/// fetched. A calibration source is wanted for the span the soil estimate trains on.
// @spec WX-REC-STORE-008, WX-EST-CAL-007
pub fn wanted_for(
    source: &Source,
    today: NaiveDate,
    located_on: NaiveDate,
) -> Option<(Product, NaiveDate)> {
    let product = product_of(source)?;
    let from = match source.role {
        SourceRole::Calibration => train_from(),
        SourceRole::Basis => wanted_from(product, today, located_on),
    };
    Some((product, from))
}

/// The fetches wanted for the plan's sources at `now`: every current fetch that is due,
/// then every span not yet held, newest first.
// @spec WX-REC-FILL-002, WX-REC-FILL-004
pub fn fetches(
    sources: &[(Source, Option<FetchState>)],
    today: NaiveDate,
    now: DateTime<Utc>,
    minute: u32,
) -> Vec<Fetch> {
    let prism_through = sources
        .iter()
        .find(|(source, _)| product_of(source) == Some(Product::PrismDays))
        .and_then(|(_, state)| state.as_ref()?.held.map(|(_, through)| through));
    let mut current = Vec::new();
    let mut backfill = Vec::new();
    for (source, state) in sources {
        let (Some(product), Some(state)) = (product_of(source), state) else {
            continue;
        };
        let fetch = |span, purpose| Fetch {
            source: source.clone(),
            product,
            span,
            purpose,
        };
        let from = match product {
            Product::NrccDays => fallback_from(prism_through, today),
            _ => current_from(product, state, today),
        };
        let due = is_due(product, state.last_success, now, minute);
        if due && from <= today {
            current.push(fetch((from, today), Purpose::Current));
        }
        if product.has_history() || product == Product::AnalysisHours {
            let held_from = state.held.map_or(from, |(held_from, _)| held_from);
            backfill.extend(
                years_back(state.wanted_from, held_from)
                    .into_iter()
                    .map(|span| fetch(span, Purpose::Backfill)),
            );
        }
    }
    backfill.sort_by_key(|fetch| std::cmp::Reverse(fetch.span.1));
    current.extend(backfill);
    current
}

/// A fetch may run this much sooner than its cadence, so that one made a little late
/// does not push the next one past its minute.
const EARLY: Duration = Duration::minutes(10);

/// The newest instant at or before `now` on the installation's minute of the hour.
pub fn slot(now: DateTime<Utc>, minute: u32) -> DateTime<Utc> {
    let this_hour = now
        .with_minute(minute.min(59))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap_or(now);
    if this_hour <= now {
        this_hour
    } else {
        this_hour - Duration::hours(1)
    }
}

/// Whether a product may have something new: it has never been fetched, or its cadence
/// has passed and the installation's minute has come round since the last success.
// @spec WX-REC-FILL-002, SYNC-SCHED-005
pub fn is_due(
    product: Product,
    last_success: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    minute: u32,
) -> bool {
    last_success
        .is_none_or(|at| at < slot(now, minute) && now - at >= product.new_data_every() - EARLY)
}

/// Where a current fetch starts: far enough back to re-read what the source revises,
/// and never before what is wanted.
// @spec WX-REC-FILL-004
fn current_from(product: Product, state: &FetchState, today: NaiveDate) -> NaiveDate {
    let from = match (state.held, product) {
        (Some((_, through)), _) => through - Duration::days(product.rereads_days()),
        // A station's files are by year: the first fetch is the year so far.
        (None, Product::StationHours | Product::CalibrationDays) => {
            NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today)
        }
        (None, Product::AnalysisHours) => state.wanted_from,
        (None, _) => today - Duration::days(FIRST_GRID_DAYS),
    };
    from.max(state.wanted_from)
}

/// The fallback grid is asked only for the days PRISM has not published.
// @spec WX-SRC-CAD-002
fn fallback_from(prism_through: Option<NaiveDate>, today: NaiveDate) -> NaiveDate {
    let recent = today - Duration::days(FIRST_GRID_DAYS);
    prism_through.map_or(recent, |through| (through + Duration::days(1)).max(recent))
}

/// The days from `wanted_from` up to, not including, `held_from`, as calendar-year
/// spans, newest first.
fn years_back(wanted_from: NaiveDate, held_from: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    let mut spans = Vec::new();
    let mut end = held_from - Duration::days(1);
    while end >= wanted_from {
        let jan1 = NaiveDate::from_ymd_opt(end.year(), 1, 1).unwrap_or(wanted_from);
        let start = jan1.max(wanted_from);
        spans.push((start, end));
        end = start - Duration::days(1);
    }
    spans
}
