use crate::error::{Result, TurfOpsError};
use crate::models::{
    Application, ApplicationType, FracClass, GrassType, IrrigationType, LawnLocation, LawnProfile,
    LogScope, RecommendationState, Severity, SoilType,
};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::warn;

#[cfg(test)]
mod row_tests;

/// Safely convert a Serialize enum variant to its string representation for DB storage.
pub(crate) fn enum_to_db_string<T: serde::Serialize>(value: T) -> Result<String> {
    let json_val = serde_json::to_value(value)
        .map_err(|e| TurfOpsError::Internal(format!("Failed to serialize enum: {}", e)))?;
    json_val
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| TurfOpsError::Internal("Expected string enum variant".into()))
}

/// Safely convert an Option<Serialize enum> to Option<String> for DB storage.
fn opt_enum_to_db_string<T: serde::Serialize>(value: Option<T>) -> Result<Option<String>> {
    match value {
        Some(v) => Ok(Some(enum_to_db_string(v)?)),
        None => Ok(None),
    }
}

// Lawn Profile Queries

pub async fn create_lawn_profile(pool: &PgPool, profile: &LawnProfile) -> Result<i64> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO lawn_profiles
            (name, grass_type, usda_zone, soil_type, lawn_size_sqft, irrigation_type, created_at, updated_at,
             latitude, longitude, place_name, timezone)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING id
        "#,
    )
    .bind(&profile.name)
    .bind(enum_to_db_string(profile.grass_type)?)
    .bind(&profile.usda_zone)
    .bind(opt_enum_to_db_string(profile.soil_type)?)
    .bind(profile.lawn_size_sqft)
    .bind(opt_enum_to_db_string(profile.irrigation_type)?)
    .bind(profile.created_at)
    .bind(profile.updated_at)
    .bind(profile.location.as_ref().map(|l| l.latitude))
    .bind(profile.location.as_ref().map(|l| l.longitude))
    .bind(profile.location.as_ref().and_then(|l| l.place_name.clone()))
    .bind(profile.location.as_ref().map(|l| l.timezone.clone()))
    .fetch_one(pool)
    .await?;

    Ok(row)
}

// @spec APP-PROF-001
pub async fn get_default_lawn_profile(pool: &PgPool) -> Result<Option<LawnProfile>> {
    let row = sqlx::query_as::<_, LawnProfileRow>(
        "SELECT id, name, grass_type, usda_zone, soil_type, lawn_size_sqft, irrigation_type, created_at, updated_at, latitude, longitude, place_name, timezone FROM lawn_profiles ORDER BY id LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into_lawn_profile()))
}

pub async fn update_lawn_profile(pool: &PgPool, profile: &LawnProfile) -> Result<()> {
    let id = profile
        .id
        .ok_or_else(|| TurfOpsError::Internal("Profile has no ID".into()))?;

    sqlx::query(
        r#"
        UPDATE lawn_profiles SET
            name = $1, grass_type = $2, usda_zone = $3, soil_type = $4,
            lawn_size_sqft = $5, irrigation_type = $6, updated_at = $7,
            latitude = $9, longitude = $10, place_name = $11, timezone = $12,
            -- The elevation belongs to the point: a lawn that moves has none until its
            -- weather is fetched again.
            elevation_ft = CASE WHEN latitude IS DISTINCT FROM $9 OR longitude IS DISTINCT FROM $10
                                THEN NULL ELSE elevation_ft END
        WHERE id = $8
        "#,
    )
    .bind(&profile.name)
    .bind(enum_to_db_string(profile.grass_type)?)
    .bind(&profile.usda_zone)
    .bind(opt_enum_to_db_string(profile.soil_type)?)
    .bind(profile.lawn_size_sqft)
    .bind(opt_enum_to_db_string(profile.irrigation_type)?)
    .bind(Utc::now())
    .bind(id)
    .bind(profile.location.as_ref().map(|l| l.latitude))
    .bind(profile.location.as_ref().map(|l| l.longitude))
    .bind(profile.location.as_ref().and_then(|l| l.place_name.clone()))
    .bind(profile.location.as_ref().map(|l| l.timezone.clone()))
    .execute(pool)
    .await?;

    Ok(())
}

// Application Queries

pub async fn get_applications_for_profile(
    pool: &PgPool,
    profile_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<Application>> {
    let rows = sqlx::query_as::<_, ApplicationRow>(
        r#"SELECT id, lawn_profile_id, application_type, product_name, application_date,
           rate_per_1000sqft, coverage_sqft, notes, nitrogen_pct, phosphorus_pct, potassium_pct,
           plant_id, plant_name, follow_up_date, frac_classes, product_id, created_at
           FROM applications WHERE lawn_profile_id = $1 ORDER BY application_date DESC
           LIMIT $2 OFFSET $3"#,
    )
    .bind(profile_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into_application()).collect())
}

/// One page of the application log, newest first. The type filter runs in SQL so it
/// applies *before* LIMIT/OFFSET — otherwise a rare type hides behind a season of mowing.
pub async fn get_applications_page(
    pool: &PgPool,
    profile_id: i64,
    application_type: Option<ApplicationType>,
    scope: Option<LogScope>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Application>> {
    let rows = sqlx::query_as::<_, ApplicationRow>(
        r#"SELECT id, lawn_profile_id, application_type, product_name, application_date,
           rate_per_1000sqft, coverage_sqft, notes, nitrogen_pct, phosphorus_pct, potassium_pct,
           plant_id, plant_name, follow_up_date, frac_classes, product_id, created_at
           FROM applications
           WHERE lawn_profile_id = $1 AND ($2::text IS NULL OR application_type = $2)
             AND ($5::text IS NULL
                  OR ($5 = 'turf') = (plant_id IS NULL AND plant_name IS NULL))
           ORDER BY application_date DESC, id DESC
           LIMIT $3 OFFSET $4"#,
    )
    .bind(profile_id)
    .bind(opt_enum_to_db_string(application_type)?)
    .bind(limit)
    .bind(offset)
    .bind(scope.map(|s| match s {
        LogScope::Turf => "turf",
        LogScope::Landscape => "landscape",
    }))
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into_application()).collect())
}

/// Applications *applied* in `[start_date, end_date)`. Everything that reasons about what
/// was done when (nitrogen budget, disease, timing, plan) uses this.
pub async fn get_applications_for_profile_in_range(
    pool: &PgPool,
    profile_id: i64,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<Vec<Application>> {
    let rows = sqlx::query_as::<_, ApplicationRow>(
        r#"SELECT id, lawn_profile_id, application_type, product_name, application_date,
           rate_per_1000sqft, coverage_sqft, notes, nitrogen_pct, phosphorus_pct, potassium_pct,
           plant_id, plant_name, follow_up_date, frac_classes, product_id, created_at
           FROM applications
           WHERE lawn_profile_id = $1
             AND application_date >= $2 AND application_date < $3
           ORDER BY application_date DESC"#,
    )
    .bind(profile_id)
    .bind(start_date)
    .bind(end_date)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into_application()).collect())
}

/// At most `limit` applications dated in `[start, end)`, newest first, optionally of one
/// type: filtered and limited in SQL, so a wide range never reads the whole log.
// @spec MCP-TOOL-026
pub async fn get_applications_in_range_page(
    pool: &PgPool,
    profile_id: i64,
    application_type: Option<ApplicationType>,
    start_date: NaiveDate,
    end_date: NaiveDate,
    limit: i64,
) -> Result<Vec<Application>> {
    let rows = sqlx::query_as::<_, ApplicationRow>(
        r#"SELECT id, lawn_profile_id, application_type, product_name, application_date,
           rate_per_1000sqft, coverage_sqft, notes, nitrogen_pct, phosphorus_pct, potassium_pct,
           plant_id, plant_name, follow_up_date, frac_classes, product_id, created_at
           FROM applications
           WHERE lawn_profile_id = $1
             AND application_date >= $2 AND application_date < $3
             AND ($4::text IS NULL OR application_type = $4)
           ORDER BY application_date DESC, id DESC
           LIMIT $5"#,
    )
    .bind(profile_id)
    .bind(start_date)
    .bind(end_date)
    .bind(opt_enum_to_db_string(application_type)?)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into_application()).collect())
}

/// Applications applied in the range **or** with a follow-up due in it — the calendar
/// shows both. Rows matched only by follow-up carry an `application_date` outside the range.
pub async fn get_applications_or_follow_ups_in_range(
    pool: &PgPool,
    profile_id: i64,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<Vec<Application>> {
    let rows = sqlx::query_as::<_, ApplicationRow>(
        r#"SELECT id, lawn_profile_id, application_type, product_name, application_date,
           rate_per_1000sqft, coverage_sqft, notes, nitrogen_pct, phosphorus_pct, potassium_pct,
           plant_id, plant_name, follow_up_date, frac_classes, product_id, created_at
           FROM applications
           WHERE lawn_profile_id = $1
             AND (
               (application_date >= $2 AND application_date < $3)
               OR (follow_up_date >= $2 AND follow_up_date < $3)
             )
           ORDER BY application_date DESC"#,
    )
    .bind(profile_id)
    .bind(start_date)
    .bind(end_date)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into_application()).collect())
}

pub async fn create_application(pool: &PgPool, app: &Application) -> Result<i64> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO applications
            (lawn_profile_id, application_type, product_name, application_date,
             rate_per_1000sqft, coverage_sqft, notes,
             nitrogen_pct, phosphorus_pct, potassium_pct, plant_id, plant_name, follow_up_date,
             frac_classes, product_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        RETURNING id
        "#,
    )
    .bind(app.lawn_profile_id)
    .bind(enum_to_db_string(app.application_type)?)
    .bind(&app.product_name)
    .bind(app.application_date)
    .bind(app.rate_per_1000sqft)
    .bind(app.coverage_sqft)
    .bind(&app.notes)
    .bind(app.nitrogen_pct)
    .bind(app.phosphorus_pct)
    .bind(app.potassium_pct)
    .bind(app.plant_id)
    .bind(&app.plant_name)
    .bind(app.follow_up_date)
    .bind(frac_classes_to_db(&app.frac_classes)?)
    .bind(app.product_id)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn update_application(pool: &PgPool, app: &Application) -> Result<u64> {
    let id = app.id.ok_or_else(|| {
        TurfOpsError::Internal("update_application requires Application.id".into())
    })?;
    let result = sqlx::query(
        r#"
        UPDATE applications
           SET application_type = $2,
               product_name = $3,
               application_date = $4,
               rate_per_1000sqft = $5,
               coverage_sqft = $6,
               notes = $7,
               nitrogen_pct = $8,
               phosphorus_pct = $9,
               potassium_pct = $10,
               plant_id = $11,
               plant_name = $12,
               follow_up_date = $13,
               frac_classes = $14,
               product_id = $15
         WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(enum_to_db_string(app.application_type)?)
    .bind(&app.product_name)
    .bind(app.application_date)
    .bind(app.rate_per_1000sqft)
    .bind(app.coverage_sqft)
    .bind(&app.notes)
    .bind(app.nitrogen_pct)
    .bind(app.phosphorus_pct)
    .bind(app.potassium_pct)
    .bind(app.plant_id)
    .bind(&app.plant_name)
    .bind(app.follow_up_date)
    .bind(frac_classes_to_db(&app.frac_classes)?)
    .bind(app.product_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn get_application_by_id(pool: &PgPool, id: i64) -> Result<Option<Application>> {
    let row = sqlx::query_as::<_, ApplicationRow>(
        r#"SELECT id, lawn_profile_id, application_type, product_name, application_date,
                  rate_per_1000sqft, coverage_sqft, notes, nitrogen_pct, phosphorus_pct, potassium_pct,
                  plant_id, plant_name, follow_up_date, frac_classes, product_id, created_at
           FROM applications WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.into_application()))
}

/// Rows deleted (0 when the id is unknown).
pub async fn delete_application(pool: &PgPool, id: i64) -> Result<u64> {
    let result = sqlx::query("DELETE FROM applications WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

// Recommendation State Queries

pub async fn get_recommendation_states(
    pool: &PgPool,
) -> Result<std::collections::HashMap<String, RecommendationState>> {
    let rows = sqlx::query_as::<_, (String, bool, bool, Option<String>, DateTime<Utc>)>(
        "SELECT id, dismissed, addressed, severity, updated_at FROM recommendation_states",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, dismissed, addressed, severity, updated_at)| {
            let state = RecommendationState {
                dismissed,
                addressed,
                severity: severity.and_then(|s| Severity::from_str(&s).ok()),
                updated_at,
            };
            (id, state)
        })
        .collect())
}

// @spec FEED-STATE-001
/// Record an answer. `severity` is what the recommendation showed when the user clicked.
pub async fn upsert_recommendation_state(
    pool: &PgPool,
    id: &str,
    dismissed: bool,
    addressed: bool,
    severity: Option<Severity>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO recommendation_states (id, dismissed, addressed, severity, updated_at)
        VALUES ($1, $2, $3, $4, NOW())
        ON CONFLICT (id) DO UPDATE SET
            dismissed = $2,
            addressed = $3,
            severity = $4,
            updated_at = NOW()
        "#,
    )
    .bind(id)
    .bind(dismissed)
    .bind(addressed)
    .bind(severity.map(|s| s.as_str()))
    .execute(pool)
    .await?;

    Ok(())
}

/// Forget an answer (the "Restore" action).
pub async fn delete_recommendation_state(pool: &PgPool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM recommendation_states WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// Row types for sqlx mapping

#[derive(sqlx::FromRow)]
struct LawnProfileRow {
    id: i64,
    name: String,
    grass_type: String,
    usda_zone: String,
    soil_type: Option<String>,
    lawn_size_sqft: Option<f64>,
    irrigation_type: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    place_name: Option<String>,
    timezone: Option<String>,
}

impl LawnProfileRow {
    // @spec APP-PROF-007
    fn into_lawn_profile(self) -> LawnProfile {
        let grass_type = GrassType::from_str(&self.grass_type).unwrap_or_else(|_| {
            warn!(
                grass_type = %self.grass_type,
                "Unknown grass_type in database, defaulting to TallFescue"
            );
            GrassType::TallFescue
        });
        let soil_type = self.soil_type.as_ref().and_then(|s| {
            SoilType::from_str(s).ok().or_else(|| {
                warn!(soil_type = %s, "Unknown soil_type in database, ignoring");
                None
            })
        });
        let irrigation_type = self.irrigation_type.as_ref().and_then(|i| {
            IrrigationType::from_str(i).ok().or_else(|| {
                warn!(irrigation_type = %i, "Unknown irrigation_type in database, ignoring");
                None
            })
        });

        LawnProfile {
            id: Some(self.id),
            name: self.name,
            grass_type,
            usda_zone: self.usda_zone,
            soil_type,
            lawn_size_sqft: self.lawn_size_sqft,
            irrigation_type,
            // @spec APP-PROF-013
            location: match (self.latitude, self.longitude, self.timezone) {
                (Some(latitude), Some(longitude), Some(timezone)) => Some(LawnLocation {
                    latitude,
                    longitude,
                    place_name: self.place_name,
                    timezone,
                }),
                _ => None,
            },
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

fn frac_classes_to_db(classes: &Option<Vec<FracClass>>) -> Result<Option<Vec<String>>> {
    classes
        .as_ref()
        .map(|list| list.iter().map(|c| enum_to_db_string(*c)).collect())
        .transpose()
}

/// Unknown codes are dropped (with a warning) rather than failing the whole row.
// @spec APP-LOG-008
fn parse_frac_classes(stored: Option<Vec<String>>) -> Option<Vec<FracClass>> {
    stored.map(|codes| {
        codes
            .into_iter()
            .filter_map(|code| {
                let parsed = serde_json::from_value(serde_json::Value::String(code.clone())).ok();
                if parsed.is_none() {
                    warn!(frac_class = %code, "Unknown frac_class in database, ignoring");
                }
                parsed
            })
            .collect()
    })
}

#[derive(sqlx::FromRow)]
struct ApplicationRow {
    id: i64,
    lawn_profile_id: i64,
    application_type: String,
    product_name: Option<String>,
    application_date: NaiveDate,
    rate_per_1000sqft: Option<f64>,
    coverage_sqft: Option<f64>,
    notes: Option<String>,
    nitrogen_pct: Option<f64>,
    phosphorus_pct: Option<f64>,
    potassium_pct: Option<f64>,
    plant_id: Option<i64>,
    plant_name: Option<String>,
    follow_up_date: Option<NaiveDate>,
    frac_classes: Option<Vec<String>>,
    product_id: Option<i64>,
    created_at: DateTime<Utc>,
}

impl ApplicationRow {
    // @spec APP-LOG-008
    fn into_application(self) -> Application {
        let application_type =
            ApplicationType::from_str(&self.application_type).unwrap_or_else(|_| {
                warn!(
                    application_type = %self.application_type,
                    "Unknown application_type in database, defaulting to Other"
                );
                ApplicationType::Other
            });

        Application {
            id: Some(self.id),
            lawn_profile_id: self.lawn_profile_id,
            application_type,
            product_name: self.product_name,
            application_date: self.application_date,
            rate_per_1000sqft: self.rate_per_1000sqft,
            coverage_sqft: self.coverage_sqft,
            notes: self.notes,
            plant_name: self.plant_name,
            nitrogen_pct: self.nitrogen_pct,
            phosphorus_pct: self.phosphorus_pct,
            potassium_pct: self.potassium_pct,
            plant_id: self.plant_id,
            follow_up_date: self.follow_up_date,
            frac_classes: parse_frac_classes(self.frac_classes),
            product_id: self.product_id,
            created_at: self.created_at,
        }
    }
}
