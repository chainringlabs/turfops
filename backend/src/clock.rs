//! The app's one clock. "Today", every date gate and every year boundary are the lawn's
//! local calendar day, in the timezone of the lawn's location, or the host's default zone
//! while the lawn has none. Everything that needs a local date takes a `Clock` (or a
//! `today` computed from one) rather than reading a clock of its own; the app's clock is
//! shared by every request (`AppClock`) and replaced when the location changes.

use crate::models::LawnProfile;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::Tz;
use std::sync::{Arc, RwLock};

/// Where local dates come from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Clock {
    /// The zone of the lawn's location.
    Zone(Tz),
    /// The host's default zone (UTC in the container image): the lawn has no location.
    #[default]
    System,
    /// Stopped at one instant in one zone. Configuration never produces it; tests use it
    /// to run a whole request on a chosen day.
    #[cfg_attr(not(test), allow(dead_code))]
    Frozen { at: DateTime<Utc>, zone: Tz },
}

/// One reading of the clock: the instant and its local date, so a computation that needs
/// both judges them on the same reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moment {
    pub now: DateTime<Utc>,
    pub today: NaiveDate,
}

impl Clock {
    // @spec DEPLOY-TZ-002
    /// A lawn's clock: its location's zone, or the host's default zone without one.
    pub fn of_lawn(profile: Option<&LawnProfile>) -> Self {
        profile
            .and_then(|p| p.location.as_ref())
            .and_then(|location| location.timezone.parse::<Tz>().ok())
            .map_or(Clock::System, Clock::Zone)
    }

    /// The local wall-clock time of an instant.
    pub fn local(&self, instant: DateTime<Utc>) -> NaiveDateTime {
        match self {
            Clock::Zone(tz) | Clock::Frozen { zone: tz, .. } => {
                instant.with_timezone(tz).naive_local()
            }
            Clock::System => instant.with_timezone(&Local).naive_local(),
        }
    }

    /// The current instant: the real time, or the instant a frozen clock stopped at.
    // @spec DEPLOY-TZ-007
    pub fn now(&self) -> DateTime<Utc> {
        match self {
            Clock::Frozen { at, .. } => *at,
            Clock::Zone(_) | Clock::System => Utc::now(),
        }
    }

    /// "Now" and "today" from one reading.
    // @spec DEPLOY-TZ-007
    pub fn moment(&self) -> Moment {
        let now = self.now();
        Moment {
            now,
            today: self.local_date(now),
        }
    }

    /// The local calendar date of an instant.
    pub fn local_date(&self, instant: DateTime<Utc>) -> NaiveDate {
        self.local(instant).date()
    }

    /// Today's local date.
    pub fn today(&self) -> NaiveDate {
        self.local_date(self.now())
    }

    // @spec DEPLOY-TZ-003
    /// One line for the log: the zone, its UTC offset at `instant`, the local date and
    /// time, and where the zone came from, so a wrong clock is visible at a glance.
    pub fn describe(&self, instant: DateTime<Utc>) -> String {
        let (zone, offset, from) = match self {
            Clock::Zone(tz) | Clock::Frozen { zone: tz, .. } => (
                tz.name().to_string(),
                instant.with_timezone(tz).format("%:z"),
                "the lawn's location",
            ),
            Clock::System => (
                "system default".to_string(),
                instant.with_timezone(&Local).format("%:z"),
                "the host's default zone; the lawn has no location",
            ),
        };
        format!(
            "{zone} (UTC{offset}), local time {}, from {from}",
            self.local(instant).format("%Y-%m-%d %H:%M")
        )
    }
}

/// The app's clock, shared by every request and replaced when the lawn's location
/// changes, so each request reads the clock in force when it starts.
#[derive(Clone, Debug)]
pub struct AppClock(Arc<RwLock<Clock>>);

impl AppClock {
    pub fn new(clock: Clock) -> Self {
        Self(Arc::new(RwLock::new(clock)))
    }

    /// The clock in force.
    pub fn get(&self) -> Clock {
        *self
            .0
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    // @spec DEPLOY-TZ-008, DEPLOY-TZ-003
    /// Puts `clock` in force, logging it when it differs from the one it replaces.
    pub fn replace(&self, clock: Clock) {
        let mut held = self
            .0
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *held != clock {
            *held = clock;
            tracing::info!("Clock: {}", clock.describe(Utc::now()));
        }
    }

    pub fn now(&self) -> DateTime<Utc> {
        self.get().now()
    }

    pub fn moment(&self) -> Moment {
        self.get().moment()
    }

    pub fn today(&self) -> NaiveDate {
        self.get().today()
    }

    pub fn local(&self, instant: DateTime<Utc>) -> NaiveDateTime {
        self.get().local(instant)
    }
}

#[cfg(test)]
mod frozen_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn eastern() -> Clock {
        Clock::Zone(chrono_tz::America::New_York)
    }

    fn at(utc: &str) -> DateTime<Utc> {
        utc.parse().unwrap()
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn lawn_at(latitude: f64, longitude: f64) -> LawnProfile {
        LawnProfile {
            location: Some(crate::models::LawnLocation::at(latitude, longitude, None).unwrap()),
            ..LawnProfile::default()
        }
    }

    // @spec DEPLOY-TZ-002
    #[test]
    fn the_clock_is_the_lawns_zone_or_the_host_default_without_a_location() {
        assert_eq!(Clock::of_lawn(Some(&lawn_at(39.86, -75.79))), eastern());
        assert_eq!(
            Clock::of_lawn(Some(&lawn_at(39.74, -104.99))),
            Clock::Zone(chrono_tz::America::Denver)
        );
        assert_eq!(Clock::of_lawn(Some(&LawnProfile::default())), Clock::System);
        assert_eq!(Clock::of_lawn(None), Clock::System);
    }

    // @spec DEPLOY-TZ-008
    #[test]
    fn a_replaced_clock_is_what_every_holder_reads_next() {
        let shared = AppClock::new(Clock::System);
        let elsewhere = shared.clone();
        shared.replace(eastern());
        assert_eq!(elsewhere.get(), eastern());
        let instant = at("2026-01-01T03:30:00Z");
        assert_eq!(elsewhere.get().local_date(instant), date(2025, 12, 31));
    }

    #[test]
    fn the_local_date_is_the_lawns_not_utcs() {
        // 10:30 pm Eastern on New Year's Eve is already Jan 1 in UTC.
        assert_eq!(
            eastern().local_date(at("2026-01-01T03:30:00Z")),
            date(2025, 12, 31)
        );
        // Same wall-clock evening in summer (EDT, UTC−4).
        assert_eq!(
            eastern().local_date(at("2026-07-15T03:30:00Z")),
            date(2026, 7, 14)
        );
    }

    #[test]
    fn local_time_follows_daylight_saving() {
        // DST ends 2026-11-01 at 06:00 UTC: 04:30 UTC on Nov 2 is 11:30 pm EST on Nov 1.
        let local = eastern().local(at("2026-11-02T04:30:00Z"));
        assert_eq!(local.date(), date(2026, 11, 1));
        assert_eq!(local.format("%H:%M").to_string(), "23:30");
    }

    // @spec DEPLOY-TZ-003
    #[test]
    fn the_log_line_names_the_zone_offset_local_time_and_where_the_zone_came_from() {
        let line = eastern().describe(at("2026-10-11T13:05:00Z"));
        assert!(line.contains("America/New_York"), "{line}");
        assert!(line.contains("-04:00"), "{line}");
        assert!(line.contains("2026-10-11 09:05"), "{line}");
        assert!(line.contains("from the lawn's location"), "{line}");
        let host = Clock::System.describe(at("2026-10-11T13:05:00Z"));
        assert!(host.contains("the lawn has no location"), "{host}");
    }
}
