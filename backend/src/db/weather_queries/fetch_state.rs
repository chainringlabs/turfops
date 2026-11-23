//! What each source's product is wanted for, what is held, and how its fetches have gone.
//! Rows and the span they extend are written in one step.

use super::rows;
use crate::error::Result;
use crate::models::weather_record::{DailyRow, FetchState, HourlyRow, Source};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{PgConnection, PgPool};

/// Registers a product of a source as wanted from `wanted_from`. A product already
/// registered keeps what it holds and its history; only the wanted start changes.
// @spec WX-REC-FILL-001
pub async fn want(
    pool: &PgPool,
    source_id: i64,
    product: &str,
    wanted_from: NaiveDate,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO weather_fetch_state (source_id, product, wanted_from) VALUES ($1, $2, $3)
           ON CONFLICT (source_id, product) DO UPDATE SET wanted_from = EXCLUDED.wanted_from"#,
    )
    .bind(source_id)
    .bind(product)
    .bind(wanted_from)
    .execute(pool)
    .await?;
    Ok(())
}

/// Marks a fetch successful and widens the held span to include `span`.
async fn succeeded(
    conn: &mut PgConnection,
    source_id: i64,
    product: &str,
    span: (NaiveDate, NaiveDate),
    at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE weather_fetch_state SET
               held_from = LEAST(COALESCE(held_from, $3), $3),
               held_through = GREATEST(COALESCE(held_through, $4), $4),
               last_attempt = $5, last_success = $5
           WHERE source_id = $1 AND product = $2"#,
    )
    .bind(source_id)
    .bind(product)
    .bind(span.0)
    .bind(span.1)
    .bind(at)
    .execute(conn)
    .await?;
    Ok(())
}

/// Stores a successful hourly fetch: its rows, and the span now held, together or not at all.
// @spec WX-REC-FILL-003
pub async fn hourly_fetched(
    pool: &PgPool,
    source_id: i64,
    product: &str,
    fetched: &[HourlyRow],
    span: (NaiveDate, NaiveDate),
    at: DateTime<Utc>,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    rows::write_hourly(&mut tx, source_id, fetched).await?;
    succeeded(&mut tx, source_id, product, span, at).await?;
    tx.commit().await?;
    Ok(())
}

/// Stores a successful daily fetch: its rows, and the span now held, together or not at all.
// @spec WX-REC-FILL-003
pub async fn daily_fetched(
    pool: &PgPool,
    source: &Source,
    product: &str,
    fetched: &[DailyRow],
    span: (NaiveDate, NaiveDate),
    at: DateTime<Utc>,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    rows::write_daily(&mut tx, source, fetched).await?;
    succeeded(&mut tx, source.id, product, span, at).await?;
    tx.commit().await?;
    Ok(())
}

/// Records a fetch that succeeded and brought nothing new: the source has not
/// published past what is held.
pub async fn checked(
    pool: &PgPool,
    source_id: i64,
    product: &str,
    at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE weather_fetch_state SET last_attempt = $3, last_success = $3 \
         WHERE source_id = $1 AND product = $2",
    )
    .bind(source_id)
    .bind(product)
    .bind(at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Records a failed fetch. What is held does not change.
// @spec WX-REC-FILL-003
pub async fn failed(
    pool: &PgPool,
    source_id: i64,
    product: &str,
    error: &str,
    at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE weather_fetch_state
           SET last_attempt = $3, last_failure = $3, last_error = $4
           WHERE source_id = $1 AND product = $2"#,
    )
    .bind(source_id)
    .bind(product)
    .bind(at)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct StateRow {
    source_id: i64,
    product: String,
    wanted_from: NaiveDate,
    held_from: Option<NaiveDate>,
    held_through: Option<NaiveDate>,
    last_attempt: Option<DateTime<Utc>>,
    last_success: Option<DateTime<Utc>>,
    last_failure: Option<DateTime<Utc>>,
    last_error: Option<String>,
}

/// Every source's fetch state, by source and product.
// @spec WX-REC-FILL-001
pub async fn all(pool: &PgPool) -> Result<Vec<FetchState>> {
    let rows = sqlx::query_as::<_, StateRow>(
        r#"SELECT source_id, product, wanted_from, held_from, held_through, last_attempt,
                  last_success, last_failure, last_error
           FROM weather_fetch_state ORDER BY source_id, product"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| FetchState {
            source_id: r.source_id,
            product: r.product,
            wanted_from: r.wanted_from,
            held: r.held_from.zip(r.held_through),
            last_attempt: r.last_attempt,
            last_success: r.last_success,
            last_failure: r.last_failure,
            last_error: r.last_error,
        })
        .collect())
}
