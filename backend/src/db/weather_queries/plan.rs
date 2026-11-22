//! The stored basis plan: for each input, the sources asked, in order.

use super::sources;
use crate::error::{Result, TurfOpsError};
use crate::logic::weather_record::plan::Ranked;
use crate::models::weather_record::{Input, Source};
use sqlx::PgPool;
use std::collections::HashMap;

/// One place in the stored plan.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub input: Input,
    pub rank: i32,
    pub source: Source,
    pub pinned: bool,
}

/// Stores `plan` as the plan in force, creating the sources it names. A source already
/// known keeps its id, and so its rows. The plan changes in one step.
// @spec WX-REC-PLAN-002
pub async fn replace(pool: &PgPool, plan: &[Ranked]) -> Result<()> {
    let mut ids: HashMap<&str, i64> = HashMap::new();
    for ranked in plan {
        if !ids.contains_key(ranked.source.key.as_str()) {
            let stored = sources::ensure(pool, &ranked.source).await?;
            ids.insert(&ranked.source.key, stored.id);
        }
    }
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM weather_basis_plan")
        .execute(&mut *tx)
        .await?;
    for ranked in plan {
        sqlx::query(
            "INSERT INTO weather_basis_plan (input, rank, source_id, pinned) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(ranked.input.as_str())
        .bind(ranked.rank)
        .bind(ids[ranked.source.key.as_str()])
        .bind(ranked.pinned)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// The plan in force, by input and then rank.
pub async fn read(pool: &PgPool) -> Result<Vec<Placed>> {
    let rows: Vec<(String, i32, i64, bool)> = sqlx::query_as(
        "SELECT input, rank, source_id, pinned FROM weather_basis_plan ORDER BY input, rank",
    )
    .fetch_all(pool)
    .await?;
    let by_id: HashMap<i64, Source> = sources::all(pool)
        .await?
        .into_iter()
        .map(|source| (source.id, source))
        .collect();
    rows.into_iter()
        .map(|(input, rank, source_id, pinned)| {
            let unknown = || TurfOpsError::Internal(format!("basis plan input {input:?}"));
            Ok(Placed {
                input: Input::parse(&input).ok_or_else(unknown)?,
                rank,
                source: by_id.get(&source_id).cloned().ok_or_else(unknown)?,
                pinned,
            })
        })
        .collect()
}
