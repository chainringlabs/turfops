//! What a value from the weather record rests on (WX-REC-BASIS). No contract returns a
//! number without it.

use super::weather_record::{Input, Source, SourceKind};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BasisKind {
    /// The owner's own sensor.
    Sensor,
    /// A measured station.
    Station,
    /// A grid, the hourly analysis or the soil estimate.
    Estimate,
}

/// The source behind a value.
// @spec WX-REC-BASIS-001, WX-REC-BASIS-002
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Basis {
    pub kind: BasisKind,
    /// The source's name, as shown to the owner.
    pub source: String,
    /// How far a station is from the lawn.
    pub distance_miles: Option<f64>,
    /// Set for the hourly analysis, a model value at the point.
    pub approximate: bool,
}

impl From<&Source> for Basis {
    fn from(source: &Source) -> Self {
        let kind = match source.kind {
            SourceKind::Sensor => BasisKind::Sensor,
            SourceKind::Station => BasisKind::Station,
            SourceKind::Grid | SourceKind::Analysis | SourceKind::Estimate => BasisKind::Estimate,
        };
        Basis {
            kind,
            source: source.name.clone(),
            distance_miles: source.distance_miles.filter(|_| kind == BasisKind::Station),
            approximate: source.kind == SourceKind::Analysis,
        }
    }
}

/// A run of an input's days in a series that rest on one basis.
// @spec WX-REC-BASIS-003
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BasisSpan {
    pub input: Input,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub basis: Basis,
}
