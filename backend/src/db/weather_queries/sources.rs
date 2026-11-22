//! The sources in use for the lawn.

use crate::error::{Result, TurfOpsError};
use crate::models::weather_record::{NewSource, Source, SourceKind, SourceRole};
use sqlx::PgPool;

macro_rules! columns {
    () => {
        "id, key, kind, name, station_ref, latitude, longitude, distance_miles, role"
    };
}

#[derive(sqlx::FromRow)]
struct SourceRow {
    id: i64,
    key: String,
    kind: String,
    name: String,
    station_ref: Option<i64>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    distance_miles: Option<f64>,
    role: String,
}

impl SourceRow {
    fn into_source(self) -> Result<Source> {
        let unknown = |what: &str, value: &str| {
            TurfOpsError::Internal(format!(
                "weather source {} has unknown {what} {value:?}",
                self.id
            ))
        };
        Ok(Source {
            kind: SourceKind::parse(&self.kind).ok_or_else(|| unknown("kind", &self.kind))?,
            role: SourceRole::parse(&self.role).ok_or_else(|| unknown("role", &self.role))?,
            id: self.id,
            key: self.key,
            name: self.name,
            station_ref: self.station_ref,
            latitude: self.latitude,
            longitude: self.longitude,
            distance_miles: self.distance_miles,
        })
    }
}

/// The source with this key, created when new and brought up to date when known. Its id,
/// and so every row stored under it, is kept.
// @spec WX-REC-STORE-001, WX-REC-PLAN-009
pub async fn ensure(pool: &PgPool, source: &NewSource) -> Result<Source> {
    let row = sqlx::query_as::<_, SourceRow>(concat!(
        "INSERT INTO weather_source \
             (key, kind, name, station_ref, latitude, longitude, distance_miles, role) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (key) DO UPDATE SET \
             kind = EXCLUDED.kind, name = EXCLUDED.name, station_ref = EXCLUDED.station_ref, \
             latitude = EXCLUDED.latitude, longitude = EXCLUDED.longitude, \
             distance_miles = EXCLUDED.distance_miles, role = EXCLUDED.role \
         RETURNING ",
        columns!()
    ))
    .bind(&source.key)
    .bind(source.kind.as_str())
    .bind(&source.name)
    .bind(source.station_ref)
    .bind(source.latitude)
    .bind(source.longitude)
    .bind(source.distance_miles)
    .bind(source.role.as_str())
    .fetch_one(pool)
    .await?;
    row.into_source()
}

pub async fn all(pool: &PgPool) -> Result<Vec<Source>> {
    sqlx::query_as::<_, SourceRow>(concat!(
        "SELECT ",
        columns!(),
        " FROM weather_source ORDER BY id"
    ))
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(SourceRow::into_source)
    .collect()
}

/// Removes a source and, with it, every row stored under it.
pub async fn delete(pool: &PgPool, source_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM weather_source WHERE id = $1")
        .bind(source_id)
        .execute(pool)
        .await?;
    Ok(())
}
