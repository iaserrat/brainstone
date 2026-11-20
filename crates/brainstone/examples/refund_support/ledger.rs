//! The application's simulated payment service, separate from agent records.
use serde::Serialize;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Receipt {
    pub(crate) refund_id: String,
    pub(crate) order_id: String,
    pub(crate) amount_cents: u32,
}

#[derive(Debug)]
pub(crate) enum RefundError {
    Storage(sqlx::Error),
    Conflict,
}

impl From<sqlx::Error> for RefundError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error)
    }
}

impl From<RefundError> for brainstone::ToolError {
    fn from(error: RefundError) -> Self {
        match error {
            RefundError::Storage(error) => Self::new("refund_storage_failed", error.to_string()),
            RefundError::Conflict => Self::new(
                "refund_conflict",
                "Retry key has different refund arguments.",
            ),
        }
    }
}

pub(crate) async fn open(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(url)
        .await?;
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA synchronous=FULL")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS demo_refunds(retry_key TEXT PRIMARY KEY,order_id TEXT NOT NULL,amount_cents INTEGER NOT NULL)")
        .execute(&pool).await?;
    Ok(pool)
}

pub(crate) async fn record(
    pool: &SqlitePool,
    key: &str,
    order: &str,
    amount: u32,
) -> Result<Receipt, RefundError> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO demo_refunds VALUES(?,?,?) ON CONFLICT DO NOTHING")
        .bind(key)
        .bind(order)
        .bind(amount)
        .execute(&mut *tx)
        .await?;
    let stored: (String, u32) =
        sqlx::query_as("SELECT order_id,amount_cents FROM demo_refunds WHERE retry_key=?")
            .bind(key)
            .fetch_one(&mut *tx)
            .await?;
    if stored != (order.to_owned(), amount) {
        return Err(RefundError::Conflict);
    }
    tx.commit().await?;
    Ok(Receipt {
        refund_id: key.to_owned(),
        order_id: stored.0,
        amount_cents: stored.1,
    })
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
