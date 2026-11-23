//! What the record keeps about the lawn's point beyond the profile's own fields.

use crate::error::Result;
use sqlx::PgPool;

/// The lawn's elevation in feet, once the hourly analysis has reported it.
pub async fn elevation_ft(pool: &PgPool) -> Result<Option<f64>> {
    let held: Option<Option<f64>> =
        sqlx::query_scalar("SELECT elevation_ft FROM lawn_profiles ORDER BY id LIMIT 1")
            .fetch_optional(pool)
            .await?;
    Ok(held.flatten())
}

/// Records the lawn's elevation. Returns whether it changed what was held.
pub async fn set_elevation_ft(pool: &PgPool, feet: f64) -> Result<bool> {
    let changed = sqlx::query(
        "UPDATE lawn_profiles SET elevation_ft = $1 \
         WHERE id = (SELECT id FROM lawn_profiles ORDER BY id LIMIT 1) \
           AND elevation_ft IS DISTINCT FROM $1",
    )
    .bind(feet)
    .execute(pool)
    .await?;
    Ok(changed.rows_affected() > 0)
}
