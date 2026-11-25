//! Data sync's state with no I/O: whether the sensor part is stale, how a refresh's
//! reads become the summary, and what the summary says at a given moment. The service gathers every
//! read first and then stores them here in one step, so a refresh dropped partway
//! changes nothing.

use crate::error::Result;
use crate::models::{EnvironmentalReading, EnvironmentalSummary, WeatherForecast};
use chrono::{DateTime, Duration, Utc};

/// Sensor data (weather lake + Home Assistant) goes stale after this.
pub(crate) const SENSOR_STALE: Duration = Duration::minutes(5);
/// The lake trails the station by about a day. Past this, the newest reading no longer
/// describes today's soil.
pub(crate) const SOIL_MAX_AGE: Duration = Duration::hours(72);
/// A forecast older than this is never served.
pub(crate) const FORECAST_MAX_AGE: Duration = Duration::hours(6);
/// A Home Assistant value is held through failed reads for this long.
pub(crate) const AMBIENT_HOLD: Duration = Duration::hours(1);

/// One sensor refresh's reads. `None` means the source is not configured.
pub(crate) struct SensorReads {
    pub lake: Option<Result<EnvironmentalSummary>>,
    pub home_assistant: Option<Result<EnvironmentalReading>>,
    pub gdd: Option<Result<Option<f64>>>,
}

/// A Home Assistant value and when it was read.
#[derive(Debug, Clone, Copy)]
struct Held {
    value: f64,
    read_at: DateTime<Utc>,
}

impl Held {
    /// The new reading when there is one, else the last one.
    fn update(last: Option<Held>, value: Option<f64>, now: DateTime<Utc>) -> Option<Held> {
        value
            .map(|value| Held {
                value,
                read_at: now,
            })
            .or(last)
    }

    fn within_hold(this: Option<Held>, now: DateTime<Utc>) -> Option<f64> {
        this.filter(|h| now - h.read_at < AMBIENT_HOLD)
            .map(|h| h.value)
    }
}

#[derive(Default)]
pub(crate) struct SyncState {
    /// The last good lake read, kept through failed reads (SYNC-FAIL-002).
    lake: Option<EnvironmentalSummary>,
    gdd: Option<f64>,
    ambient: Option<Held>,
    humidity: Option<Held>,
    /// The sensor part as composed by the last sensor refresh.
    sensors: EnvironmentalSummary,
    sensors_at: Option<DateTime<Utc>>,
}

impl SyncState {
    /// Whether the sensor part is stale at `now`.
    // @spec SYNC-REFRESH-002
    pub fn due(&self, now: DateTime<Utc>) -> bool {
        self.sensors_at.is_none_or(|t| now - t >= SENSOR_STALE)
    }

    /// Store a completed sensor refresh: every read is already in hand, so nothing is
    /// marked fresh before the summary it produced is stored.
    // @spec SYNC-REFRESH-008, SYNC-MERGE-001, SYNC-MERGE-002, SYNC-MERGE-003, SYNC-MERGE-004
    // @spec SYNC-MERGE-006, SYNC-FAIL-002, SYNC-FAIL-003, SYNC-FAIL-004, SYNC-FAIL-005, SYNC-FAIL-009
    pub fn store_sensors(&mut self, now: DateTime<Utc>, reads: SensorReads) {
        let lake_read = match reads.lake {
            Some(Ok(lake)) => {
                self.lake = Some(lake);
                true
            }
            Some(Err(e)) => {
                tracing::warn!("Failed to read the weather lake, keeping the last read: {e}");
                false
            }
            None => false,
        };
        match reads.gdd {
            Some(Ok(gdd)) => self.gdd = gdd,
            Some(Err(e)) => {
                tracing::warn!("Failed to read year-to-date GDD: {e}");
                if lake_read {
                    self.gdd = None;
                }
            }
            None => self.gdd = None,
        }
        match reads.home_assistant {
            Some(Ok(patio)) => {
                self.ambient = Held::update(self.ambient, patio.ambient_temp_f, now);
                self.humidity = Held::update(self.humidity, patio.humidity_percent, now);
            }
            Some(Err(e)) => tracing::warn!("Failed to read Home Assistant: {e}"),
            None => {
                self.ambient = None;
                self.humidity = None;
            }
        }
        self.sensors = self.compose(now);
        self.sensors_at = Some(now);
    }

    /// The sensor part from the last good reads, judged at `now`.
    fn compose(&self, now: DateTime<Utc>) -> EnvironmentalSummary {
        let mut summary = self.lake.clone().unwrap_or_default();
        let lake_current = summary.current.take();
        let observed = lake_current.as_ref().map(|c| c.timestamp);
        let stale = observed.is_some_and(|o| now - o > SOIL_MAX_AGE);

        let mut current = EnvironmentalReading::new(crate::models::DataSource::Cached);
        if stale {
            // The station feed has stopped: a days-old reading must not drive today's advice.
            tracing::warn!(
                ?observed,
                "Station soil data is stale; not using it as current"
            );
            summary.soil_temp_7day_avg_f = None;
            summary.ambient_temp_7day_avg_f = None;
            summary.humidity_7day_avg = None;
            summary.precipitation_7day_total_mm = None;
            summary.soil_temp_trend = crate::models::Trend::Unknown;
        } else if let Some(lake) = lake_current {
            current.soil_temp_5_f = lake.soil_temp_5_f;
            current.soil_temp_10_f = lake.soil_temp_10_f;
            current.soil_temp_20_f = lake.soil_temp_20_f;
            current.soil_temp_50_f = lake.soil_temp_50_f;
            current.soil_temp_100_f = lake.soil_temp_100_f;
            current.soil_moisture_5 = lake.soil_moisture_5;
            current.soil_moisture_10 = lake.soil_moisture_10;
            current.soil_moisture_20 = lake.soil_moisture_20;
            current.soil_moisture_50 = lake.soil_moisture_50;
            current.soil_moisture_100 = lake.soil_moisture_100;
            current.precipitation_mm = lake.precipitation_mm;
        }
        current.ambient_temp_f = Held::within_hold(self.ambient, now);
        current.humidity_percent = Held::within_hold(self.humidity, now);
        current.timestamp = now;

        EnvironmentalSummary {
            current: Some(current),
            last_updated: Some(now),
            soil_observed_at: observed,
            soil_stale: stale,
            gdd_base50_ytd: self.gdd,
            forecast: None,
            ..summary
        }
    }

    /// Move the sensor part's last refresh `by` into the past (tests).
    #[cfg(test)]
    pub(crate) fn age_sensors(&mut self, by: Duration) {
        self.sensors_at = self.sensors_at.map(|t| t - by);
    }

    /// The summary as of `now`: the stored sensor part plus `forecast`, the newest the
    /// schedule holds, while it is under `FORECAST_MAX_AGE` old.
    // @spec SYNC-FAIL-006, SYNC-FAIL-007
    pub fn summary(
        &self,
        now: DateTime<Utc>,
        forecast: Option<WeatherForecast>,
    ) -> EnvironmentalSummary {
        EnvironmentalSummary {
            forecast: forecast.filter(|f| now - f.fetched_at < FORECAST_MAX_AGE),
            ..self.sensors.clone()
        }
    }
}
