//! The basis plan: for each input, the ranked sources the record may answer from. The
//! owner's sensor first, then measured stations inside the input's cut-off, then the
//! input's estimate.

#[cfg(test)]
#[path = "plan_preview_tests.rs"]
mod preview_tests;
#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;

use crate::geo::miles_between;
use crate::models::weather_record::{Input, Network, NewSource, SourceKind, SourceRole, Station};

/// A station this close may speak for soil temperature and soil moisture.
pub const SOIL_STATION_MILES: f64 = 50.0;
/// A station this close may speak for hourly air temperature and humidity.
pub const AIR_STATION_MILES: f64 = 10.0;
/// A default station further than this above or below the lawn is passed over.
pub const ELEVATION_GUARD_FT: f64 = 1000.0;
/// The owner may pin a station within this distance.
pub const PIN_RANGE_MILES: f64 = 150.0;

/// Where the lawn is. The elevation is the one the hourly analysis reports for the
/// point, unknown until its first fetch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lawn {
    pub latitude: f64,
    pub longitude: f64,
    pub elevation_ft: Option<f64>,
}

/// One of the owner's sensors and the input it is mapped to.
#[derive(Debug, Clone, PartialEq)]
pub struct MappedSensor {
    pub input: Input,
    pub key: String,
    pub name: String,
}

/// A station the owner chose for an input, by its catalog row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pin {
    pub input: Input,
    pub station_ref: i64,
}

/// One place in the plan.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked {
    pub input: Input,
    /// 1 is asked first.
    pub rank: i32,
    pub source: NewSource,
    pub pinned: bool,
}

/// A catalog station as seen from a point.
#[derive(Debug, Clone, PartialEq)]
pub struct Nearby {
    pub station_ref: i64,
    pub network: Network,
    pub name: String,
    pub state: String,
    pub distance_miles: f64,
    /// The station's elevation less the point's; unknown when either is.
    pub elevation_difference_ft: Option<f64>,
    pub inputs: Vec<Input>,
}

/// What the record would use at a point, and the stations around it.
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    pub plan: Vec<Ranked>,
    pub nearby: Vec<Nearby>,
}

/// The plan for a lawn. Without a location there is none.
// @spec WX-REC-PLAN-003, WX-REC-PLAN-011
pub fn build(
    lawn: Option<&Lawn>,
    catalog: &[(i64, Station)],
    sensors: &[MappedSensor],
    pins: &[Pin],
) -> Vec<Ranked> {
    let Some(lawn) = lawn else {
        return Vec::new();
    };
    let near = nearby(lawn, catalog, f64::INFINITY);
    let mut plan = Vec::new();
    for input in Input::ALL {
        let mut sources: Vec<(NewSource, bool)> = Vec::new();
        sources.extend(sensor_for(*input, sensors).map(|s| (s, false)));
        match pinned(*input, pins, &near, catalog) {
            Some(station) => sources.push((station, true)),
            None => sources.extend(default_stations(*input, lawn, &near, catalog)),
        }
        sources.extend(estimates(*input, lawn).into_iter().map(|s| (s, false)));
        plan.extend(
            sources
                .into_iter()
                .zip(1..)
                .map(|((source, pinned), rank)| Ranked {
                    input: *input,
                    rank,
                    source,
                    pinned,
                }),
        );
    }
    plan
}

/// The plan a point would get and the stations within pinning range of it.
// @spec WX-REC-PLAN-012
pub fn preview(lawn: &Lawn, catalog: &[(i64, Station)]) -> Preview {
    Preview {
        plan: build(Some(lawn), catalog, &[], &[]),
        nearby: nearby(lawn, catalog, PIN_RANGE_MILES),
    }
}

/// Catalog stations within `within_miles` of the lawn, nearest first.
// @spec WX-REC-PLAN-009
pub fn nearby(lawn: &Lawn, catalog: &[(i64, Station)], within_miles: f64) -> Vec<Nearby> {
    let mut near: Vec<Nearby> = catalog
        .iter()
        .map(|(station_ref, station)| Nearby {
            station_ref: *station_ref,
            network: station.network,
            name: station.name.clone(),
            state: station.state.clone(),
            distance_miles: miles_between(
                lawn.latitude,
                lawn.longitude,
                station.latitude,
                station.longitude,
            ),
            elevation_difference_ft: station
                .elevation_ft
                .zip(lawn.elevation_ft)
                .map(|(station, lawn)| station - lawn),
            inputs: station.inputs.clone(),
        })
        .filter(|n| n.distance_miles <= within_miles)
        .collect();
    near.sort_by(|a, b| a.distance_miles.total_cmp(&b.distance_miles));
    near
}

/// The sensor that speaks for `input`. Daily air is the day's hours of an air sensor.
fn sensor_for(input: Input, sensors: &[MappedSensor]) -> Option<NewSource> {
    let measured = match input {
        Input::DailyAir => Input::AirTemp,
        other => other,
    };
    sensors
        .iter()
        .find(|s| s.input == measured)
        .map(|s| NewSource {
            key: s.key.clone(),
            kind: SourceKind::Sensor,
            name: s.name.clone(),
            station_ref: None,
            latitude: None,
            longitude: None,
            distance_miles: None,
            role: SourceRole::Basis,
        })
}

/// The station the owner pinned for `input`, at whatever distance or elevation.
// @spec WX-REC-PLAN-008
fn pinned(
    input: Input,
    pins: &[Pin],
    near: &[Nearby],
    catalog: &[(i64, Station)],
) -> Option<NewSource> {
    let pin = pins.iter().find(|p| p.input == input)?;
    let seen = near.iter().find(|n| n.station_ref == pin.station_ref)?;
    station_source(seen, catalog)
}

/// How near a default station must be to speak for `input`; `None` where only a pinned
/// station may.
// @spec WX-REC-PLAN-004, WX-REC-PLAN-005, WX-REC-PLAN-006
fn cut_off(input: Input) -> Option<f64> {
    match input {
        Input::AirTemp | Input::Humidity => Some(AIR_STATION_MILES),
        Input::DailyAir | Input::Rain => None,
        _ => Some(SOIL_STATION_MILES),
    }
}

/// The nearest USCRN station that qualifies for `input`, then the nearest SCAN one.
// @spec WX-REC-PLAN-004, WX-REC-PLAN-005, WX-REC-PLAN-007, WX-REC-PLAN-013
fn default_stations(
    input: Input,
    lawn: &Lawn,
    near: &[Nearby],
    catalog: &[(i64, Station)],
) -> Vec<(NewSource, bool)> {
    let Some(within) = cut_off(input) else {
        return Vec::new();
    };
    // Dew point needs temperature and humidity from one instrument.
    let needed: &[Input] = match input {
        Input::AirTemp | Input::Humidity => &[Input::AirTemp, Input::Humidity],
        _ => std::slice::from_ref(&input),
    };
    let qualifies = |n: &&Nearby| {
        n.distance_miles <= within
            && needed.iter().all(|i| n.inputs.contains(i))
            && (lawn.elevation_ft.is_none()
                || n.elevation_difference_ft
                    .is_none_or(|feet| feet.abs() <= ELEVATION_GUARD_FT))
    };
    [Network::Uscrn, Network::Scan]
        .into_iter()
        .filter_map(|network| near.iter().filter(|n| n.network == network).find(qualifies))
        .filter_map(|n| station_source(n, catalog))
        .map(|source| (source, false))
        .collect()
}

fn station_source(seen: &Nearby, catalog: &[(i64, Station)]) -> Option<NewSource> {
    let (_, station) = catalog.iter().find(|(id, _)| *id == seen.station_ref)?;
    let network = match station.network {
        Network::Uscrn => "USCRN",
        Network::Scan => "SCAN",
    };
    Some(NewSource {
        key: format!(
            "station:{}:{}",
            station.network.as_str(),
            station.station_id
        ),
        kind: SourceKind::Station,
        name: format!("{}, {} ({network})", station.name, station.state),
        station_ref: Some(seen.station_ref),
        latitude: Some(station.latitude),
        longitude: Some(station.longitude),
        distance_miles: Some(seen.distance_miles),
        role: SourceRole::Basis,
    })
}

/// The estimate sources for `input`, in the order they are asked. Deep soil temperature
/// and soil moisture have none.
// @spec WX-REC-PLAN-010
fn estimates(input: Input, lawn: &Lawn) -> Vec<NewSource> {
    let at_point = |prefix: &str, kind: SourceKind, name: &str| NewSource {
        // The point is part of the key, so a moved lawn starts its own rows.
        key: format!("{prefix}:{:.4},{:.4}", lawn.latitude, lawn.longitude),
        kind,
        name: name.into(),
        station_ref: None,
        latitude: Some(lawn.latitude),
        longitude: Some(lawn.longitude),
        distance_miles: None,
        role: SourceRole::Basis,
    };
    let prism = || at_point("grid:prism", SourceKind::Grid, "PRISM daily grid");
    let nrcc = || at_point("grid:nrcc", SourceKind::Grid, "NRCC daily grid");
    let analysis = || {
        at_point(
            "analysis",
            SourceKind::Analysis,
            "Open-Meteo hourly analysis",
        )
    };
    match input {
        Input::SoilTemp5 | Input::SoilTemp10 => vec![at_point(
            "estimate",
            SourceKind::Estimate,
            "Soil temperature estimate",
        )],
        Input::AirTemp | Input::Humidity => vec![analysis()],
        Input::DailyAir => vec![prism(), nrcc()],
        Input::Rain => vec![prism(), analysis()],
        _ => Vec::new(),
    }
}
