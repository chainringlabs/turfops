//! The daily grids served by RCC-ACIS (WX-SRC-GRID): PRISM, and the NRCC interpolated
//! grid behind it. One request returns a point's daily maximum, minimum and
//! precipitation for any span.

#[cfg(test)]
mod tests;

use super::outside::{Outside, LONG_REQUEST_TIMEOUT, REQUEST_TIMEOUT};
use crate::error::{Result, Source, TurfOpsError};
use crate::models::weather_record::DailyRow;
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

const URL: &str = "https://grid2.rcc-acis.org/GridData";
/// Values at or below this are the grid's missing marker (−999).
const MISSING_BELOW: f64 = -900.0;
const MM_PER_INCH: f64 = 25.4;
const TEMPERATURE_F: (f64, f64) = (-80.0, 150.0);
const DAILY_PRECIP_MM: (f64, f64) = (0.0, 1500.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grid {
    /// PRISM daily, about 4 km, through yesterday.
    Prism,
    /// The NRCC interpolated grid, through today; used for days PRISM lacks.
    Nrcc,
}

impl Grid {
    /// The grid's number at RCC-ACIS.
    fn id(self) -> &'static str {
        match self {
            Grid::Prism => "21",
            Grid::Nrcc => "1",
        }
    }
}

/// A point's days from one grid, and the centre of the grid cell they are for.
#[derive(Debug, Clone, PartialEq)]
pub struct GridDays {
    pub cell_latitude: f64,
    pub cell_longitude: f64,
    pub days: Vec<DailyRow>,
}

#[derive(Deserialize)]
struct Body {
    meta: Meta,
    data: Vec<(String, Value, Value, Value)>,
}

#[derive(Deserialize)]
struct Meta {
    lat: f64,
    lon: f64,
}

pub struct GridClient {
    outside: Arc<Outside>,
    url: String,
}

impl GridClient {
    pub fn new(outside: Arc<Outside>) -> Self {
        Self {
            outside,
            url: URL.into(),
        }
    }

    #[cfg(test)]
    pub fn with_url(mut self, url: &str) -> Self {
        self.url = url.into();
        self
    }

    /// A point's days from `from` through `through`, in one request.
    // @spec WX-SRC-GRID-001, WX-SRC-GRID-002, WX-SRC-REQ-002, WX-SRC-REQ-006, WX-SRC-ERR-001
    pub async fn fetch(
        &self,
        grid: Grid,
        latitude: f64,
        longitude: f64,
        from: NaiveDate,
        through: NaiveDate,
    ) -> Result<GridDays> {
        let body = serde_json::json!({
            "loc": format!("{longitude},{latitude}"),
            "sdate": from.to_string(),
            "edate": through.to_string(),
            "grid": grid.id(),
            "elems": [{"name": "maxt"}, {"name": "mint"}, {"name": "pcpn"}],
            "meta": "ll",
        });
        let timeout = if (through - from).num_days() > 366 {
            LONG_REQUEST_TIMEOUT
        } else {
            REQUEST_TIMEOUT
        };
        let text = self
            .outside
            .post_json(&self.url, &body, timeout)
            .await
            .map_err(down(grid))?;
        parse(&text).map_err(down(grid))
    }
}

fn down(grid: Grid) -> impl Fn(String) -> TurfOpsError {
    move |detail| TurfOpsError::unavailable(Source::Grid(grid), detail)
}

fn within(value: f64, (low, high): (f64, f64)) -> Option<f64> {
    (value >= low && value <= high).then_some(value)
}

/// The days of a grid response. Days at the end with no value are not yet published and
/// are left out; a day with no value before a day that has one is a hole, and an error.
// @spec WX-SRC-GRID-003, WX-SRC-NORM-001, WX-SRC-NORM-002, WX-SRC-NORM-004
pub fn parse(json: &str) -> std::result::Result<GridDays, String> {
    let body: Body =
        serde_json::from_str(json).map_err(|e| format!("grid response did not parse: {e}"))?;
    let present = |value: &Value| value.as_f64().filter(|v| *v > MISSING_BELOW);
    let mut days = Vec::with_capacity(body.data.len());
    for (day, max, min, precip) in &body.data {
        let day = NaiveDate::parse_from_str(day, "%Y-%m-%d")
            .map_err(|e| format!("grid day {day:?}: {e}"))?;
        let air_max_f = present(max).and_then(|v| within(v, TEMPERATURE_F));
        let air_min_f = present(min).and_then(|v| within(v, TEMPERATURE_F));
        days.push(DailyRow {
            day,
            air_max_f,
            air_min_f,
            air_mean_f: air_max_f.zip(air_min_f).map(|(max, min)| (max + min) / 2.0),
            precip_mm: present(precip)
                .map(|inches| inches * MM_PER_INCH)
                .and_then(|v| within(v, DAILY_PRECIP_MM)),
            ..Default::default()
        });
    }
    let empty =
        |d: &DailyRow| d.air_max_f.is_none() && d.air_min_f.is_none() && d.precip_mm.is_none();
    while days.last().is_some_and(empty) {
        days.pop();
    }
    if let Some(hole) = days.iter().find(|d| empty(d)) {
        return Err(format!(
            "grid response has no value for {} inside its span",
            hole.day
        ));
    }
    Ok(GridDays {
        cell_latitude: body.meta.lat,
        cell_longitude: body.meta.lon,
        days,
    })
}
