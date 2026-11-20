//! Application-owned idempotency is separate from the framework journal.
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

pub(crate) async fn open(path: &str) -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite://{path}?mode=rwc"))
        .await?;
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA synchronous=FULL")
        .execute(&pool)
        .await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS effects(retry_key TEXT PRIMARY KEY,value TEXT NOT NULL)",
    )
    .execute(&pool)
    .await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS attempts(retry_key TEXT NOT NULL,value TEXT NOT NULL)")
        .execute(&pool)
        .await?;
    Ok(pool)
}

pub(crate) async fn record(
    pool: &SqlitePool,
    key: &str,
    value: &str,
) -> Result<String, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO attempts VALUES(?,?)")
        .bind(key)
        .bind(value)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO effects VALUES(?,?) ON CONFLICT DO NOTHING")
        .bind(key)
        .bind(value)
        .execute(&mut *tx)
        .await?;
    let stored = sqlx::query_scalar("SELECT value FROM effects WHERE retry_key=?")
        .bind(key)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(stored)
}
