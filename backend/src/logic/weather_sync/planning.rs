//! Keeping the stored basis plan in line with what it is built from: the lawn's
//! location and elevation, the station catalog, the owner's mapped sensors and pinned
//! stations.

use crate::db::setup_queries;
use crate::db::weather_queries::{catalog, lawn, plan};
use crate::error::Result;
use crate::logic::weather_record::plan::{build, Lawn, MappedSensor, Pin};
use sqlx::PgPool;
use std::collections::BTreeSet;

/// Rebuilds the plan when what it would be differs from what is stored. Returns whether
/// it changed. A lawn with no location has an empty plan.
// @spec WX-REC-PLAN-002, WX-REC-PLAN-013
pub async fn rebuild(pool: &PgPool) -> Result<bool> {
    let location = crate::db::queries::get_default_lawn_profile(pool)
        .await?
        .and_then(|profile| profile.location);
    let lawn = match &location {
        Some(location) => Some(Lawn {
            latitude: location.latitude,
            longitude: location.longitude,
            elevation_ft: lawn::elevation_ft(pool).await?,
        }),
        None => None,
    };
    let sensors = mapped_sensors(pool).await?;
    let pins: Vec<Pin> = setup_queries::pins(pool)
        .await?
        .into_iter()
        .map(|(input, station_ref)| Pin { input, station_ref })
        .collect();
    let stations = catalog::stations(pool).await?;
    let wanted = build(lawn.as_ref(), &stations, &sensors, &pins);
    let held = plan::read(pool).await?;

    let mut wanted_places: Vec<_> = wanted
        .iter()
        .map(|r| (r.input.as_str(), r.rank, r.source.key.as_str(), r.pinned))
        .collect();
    let mut held_places: Vec<_> = held
        .iter()
        .map(|p| (p.input.as_str(), p.rank, p.source.key.as_str(), p.pinned))
        .collect();
    wanted_places.sort_unstable();
    held_places.sort_unstable();
    if wanted_places == held_places {
        return Ok(false);
    }
    plan::replace(pool, &wanted).await?;
    let measured: BTreeSet<&str> = wanted
        .iter()
        .filter(|ranked| ranked.source.station_ref.is_some())
        .map(|ranked| ranked.source.name.as_str())
        .collect();
    let own: BTreeSet<&str> = sensors.iter().map(|sensor| sensor.name.as_str()).collect();
    tracing::info!(stations = ?measured, sensors = ?own, "Weather plan rebuilt");
    Ok(true)
}

/// The owner's mapped sensors, as the plan ranks them.
pub async fn mapped_sensors(pool: &PgPool) -> Result<Vec<MappedSensor>> {
    Ok(setup_queries::sensors(pool)
        .await?
        .into_iter()
        .map(|sensor| MappedSensor {
            input: sensor.input,
            key: sensor.source_key(),
            name: sensor.name,
        })
        .collect())
}
