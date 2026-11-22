//! How each input is read from a source's rows: from an hourly source's day, from a daily
//! source's row, and when the source last gave it.

use super::resolve::{self, DayAir};
use crate::db::weather_queries::days::{HourlyDay, NewestHours};
use crate::models::weather_record::{DailyRow, Input};
use chrono::{DateTime, Utc};

/// A value's valid hours on a day, and the value.
type OfHours = fn(&HourlyDay) -> (i64, Option<f64>);
/// The newest hour that holds a value.
type Newest = fn(&NewestHours) -> Option<DateTime<Utc>>;

/// How an input is read from an hourly source's day, and from a daily source's row.
pub(super) struct Reading<T> {
    pub hourly: fn(&HourlyDay) -> (i64, Option<T>),
    pub daily: fn(&DailyRow) -> Option<T>,
    /// Days between a daily source's own day and the lawn day its value belongs to.
    pub shift_days: i64,
    pub newest: fn(&NewestHours) -> Option<DateTime<Utc>>,
}

fn depth(input: Input) -> usize {
    match input {
        Input::SoilTemp5 | Input::SoilMoisture5 => 0,
        Input::SoilTemp10 | Input::SoilMoisture10 => 1,
        Input::SoilTemp20 | Input::SoilMoisture20 => 2,
        Input::SoilTemp50 | Input::SoilMoisture50 => 3,
        _ => 4,
    }
}

macro_rules! at_depth {
    ($field:ident) => {
        [
            |d| resolve::mean(&d.$field[0]),
            |d| resolve::mean(&d.$field[1]),
            |d| resolve::mean(&d.$field[2]),
            |d| resolve::mean(&d.$field[3]),
            |d| resolve::mean(&d.$field[4]),
        ]
    };
}

const SOIL_TEMP: [OfHours; 5] = at_depth!(soil_temp);
const SOIL_MOISTURE: [OfHours; 5] = at_depth!(soil_moisture);
const NEWEST_SOIL_TEMP: [Newest; 5] = [
    |n| n.soil_temp[0],
    |n| n.soil_temp[1],
    |n| n.soil_temp[2],
    |n| n.soil_temp[3],
    |n| n.soil_temp[4],
];
const NEWEST_SOIL_MOISTURE: [Newest; 5] = [
    |n| n.soil_moisture[0],
    |n| n.soil_moisture[1],
    |n| n.soil_moisture[2],
    |n| n.soil_moisture[3],
    |n| n.soil_moisture[4],
];

/// How a scalar input is read. Daily air is read by `air`.
pub(super) fn scalar(input: Input) -> Reading<f64> {
    fn stat(hourly: OfHours, newest: Newest) -> Reading<f64> {
        Reading {
            hourly,
            daily: |_| None,
            shift_days: 0,
            newest,
        }
    }
    match input {
        Input::AirTemp | Input::DailyAir => stat(|d| resolve::mean(&d.air), |n| n.air),
        Input::Humidity => stat(|d| resolve::mean(&d.humidity), |n| n.humidity),
        Input::Rain => Reading {
            hourly: |d| (d.rain_hours, d.rain_mm),
            daily: |row| row.precip_mm,
            shift_days: resolve::GRID_RAIN_SHIFT_DAYS,
            newest: |n| n.rain,
        },
        Input::SoilTemp5 => Reading {
            daily: |row| row.soil_temp_5_f,
            ..stat(SOIL_TEMP[0], NEWEST_SOIL_TEMP[0])
        },
        Input::SoilTemp10 => Reading {
            daily: |row| row.soil_temp_10_f,
            ..stat(SOIL_TEMP[1], NEWEST_SOIL_TEMP[1])
        },
        Input::SoilTemp20 | Input::SoilTemp50 | Input::SoilTemp100 => {
            stat(SOIL_TEMP[depth(input)], NEWEST_SOIL_TEMP[depth(input)])
        }
        _ => stat(
            SOIL_MOISTURE[depth(input)],
            NEWEST_SOIL_MOISTURE[depth(input)],
        ),
    }
}

pub(super) fn air() -> Reading<DayAir> {
    Reading {
        hourly: resolve::air_of_hours,
        daily: resolve::air_of_grid,
        shift_days: 0,
        newest: |n| n.air,
    }
}
