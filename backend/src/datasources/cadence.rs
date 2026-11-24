//! How often each source may have something new (WX-SRC-CAD), and how far back each
//! fetch re-reads to pick up the source's revisions. Data sync schedules from this.

use chrono::Duration;

/// One thing fetched from an outside source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Product {
    /// A station's hourly rows, from USCRN or SCAN.
    StationHours,
    /// A calibration station's daily rows.
    CalibrationDays,
    /// PRISM's daily maximum, minimum and precipitation at a point.
    PrismDays,
    /// The NRCC grid's, wanted only for days PRISM has not published.
    NrccDays,
    /// The hourly analysis at a point.
    AnalysisHours,
    Forecast,
    /// The owner's Home Assistant entities.
    SensorEntities,
    /// A network's list of stations.
    Catalog,
}

impl Product {
    /// The name the record keeps a fetch under.
    pub const fn as_str(self) -> &'static str {
        match self {
            Product::StationHours => "station_hours",
            Product::CalibrationDays => "calibration_days",
            Product::PrismDays | Product::NrccDays => "grid_days",
            Product::AnalysisHours => "analysis_hours",
            Product::Forecast => "forecast",
            Product::SensorEntities => "sensor_entities",
            Product::Catalog => "catalog",
        }
    }

    /// How long after a fetch the source may have new data.
    // @spec WX-SRC-CAD-001
    pub fn new_data_every(self) -> Duration {
        match self {
            Product::StationHours | Product::AnalysisHours | Product::Forecast => {
                Duration::hours(1)
            }
            Product::CalibrationDays | Product::PrismDays | Product::NrccDays => Duration::days(1),
            Product::SensorEntities => Duration::minutes(5),
            Product::Catalog => Duration::days(30),
        }
    }

    /// How many of the newest days a fetch reads again, for values the source revises
    /// after first publishing them.
    pub fn rereads_days(self) -> i64 {
        match self {
            Product::StationHours => 3,
            Product::CalibrationDays | Product::PrismDays => 7,
            Product::AnalysisHours => 2,
            Product::NrccDays | Product::Forecast | Product::SensorEntities | Product::Catalog => 0,
        }
    }

    /// Whether the source keeps a history worth downloading. A year of a station or of
    /// PRISM that has ended never changes and is fetched once.
    // @spec WX-SRC-CAD-001, WX-SRC-CAD-002
    pub fn has_history(self) -> bool {
        matches!(
            self,
            Product::StationHours | Product::CalibrationDays | Product::PrismDays
        )
    }
}
