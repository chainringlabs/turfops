//! The owner's sensor readings in record units (WX-SRC-HA, WX-SRC-POST): converted
//! from the unit the sensor declares, and judged against what its input can plausibly be.

use crate::models::weather_record::Input;

#[cfg(test)]
#[path = "sensor_units_tests.rs"]
mod tests;

/// What kind of quantity an input is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quantity {
    Temperature,
    Humidity,
    SoilMoisture,
    Rain,
}

fn quantity(input: Input) -> Option<Quantity> {
    use Input::*;
    Some(match input {
        AirTemp | SoilTemp5 | SoilTemp10 | SoilTemp20 | SoilTemp50 | SoilTemp100 => {
            Quantity::Temperature
        }
        Humidity => Quantity::Humidity,
        Rain => Quantity::Rain,
        SoilMoisture5 | SoilMoisture10 | SoilMoisture20 | SoilMoisture50 | SoilMoisture100 => {
            Quantity::SoilMoisture
        }
        // Daily air is derived from an air sensor's hours; no sensor is mapped to it.
        DailyAir => return None,
    })
}

/// Whether a sensor can be mapped to `input`.
pub fn mappable(input: Input) -> bool {
    quantity(input).is_some()
}

/// The units a sensor mapped to `input` may report in, as the owner would write them.
pub fn accepted_units(input: Input) -> &'static str {
    match quantity(input) {
        Some(Quantity::Temperature) => "°F or °C",
        Some(Quantity::Humidity) => "%",
        Some(Quantity::SoilMoisture) => "% or a fraction (m³/m³)",
        Some(Quantity::Rain) => "mm or in",
        None => "none",
    }
}

/// A sensor's value in the record's unit for `input`, or `None` when the unit is not
/// one the input accepts.
// @spec WX-SRC-HA-001, WX-SRC-NORM-001
pub fn convert(input: Input, value: f64, unit: Option<&str>) -> Option<f64> {
    let unit = unit.unwrap_or("").trim();
    match quantity(input)? {
        Quantity::Temperature => match unit {
            "°F" | "F" => Some(value),
            "°C" | "C" => Some(value * 9.0 / 5.0 + 32.0),
            _ => None,
        },
        Quantity::Humidity => (unit == "%").then_some(value),
        Quantity::SoilMoisture => match unit {
            "%" => Some(value / 100.0),
            "m³/m³" | "m3/m3" | "" => Some(value),
            _ => None,
        },
        Quantity::Rain => match unit {
            "mm" => Some(value),
            "in" | "inch" | "inches" => Some(value * 25.4),
            _ => None,
        },
    }
}

/// Whether a value in record units is one its input could really have.
// @spec WX-SRC-NORM-002
pub fn plausible(input: Input, value: f64) -> bool {
    let (low, high) = match quantity(input) {
        Some(Quantity::Temperature) => (-80.0, 150.0),
        Some(Quantity::Humidity) => (0.0, 100.0),
        Some(Quantity::SoilMoisture) => (0.0, 1.0),
        // A counter's running total has no ceiling worth naming.
        Some(Quantity::Rain) => (0.0, f64::MAX),
        None => return false,
    };
    (low..=high).contains(&value)
}
