use crate::Error;
use crate::sqlite_backend::sqlite_error;
use crate::sqlite_backend::sqlite_error::store;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::time::Duration;
use tokio::time::Instant;

const STARTUP_WAIT: Duration = Duration::from_secs(5);
const STARTUP_BACKOFF: Duration = Duration::from_millis(25);
const MAX_CONNECTIONS: u32 = 4;

/// Concurrent first opens may race while `SQLite` changes journal mode, before
/// the migration transaction exists. Retry only native BUSY/LOCKED failures.
pub(crate) async fn connect(options: SqliteConnectOptions) -> Result<SqlitePool, Error> {
    let deadline = Instant::now() + STARTUP_WAIT;
    loop {
        match attempt(&options, deadline).await {
            Ok(pool) => return Ok(pool),
            Err(error) if sqlite_error::busy(&error) && Instant::now() < deadline => {
                tokio::time::sleep(STARTUP_BACKOFF).await;
            }
            Err(error) => return Err(store(error)),
        }
    }
}

async fn attempt(
    options: &SqliteConnectOptions,
    deadline: Instant,
) -> Result<SqlitePool, sqlx::Error> {
    let connect = SqlitePoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect_with(options.clone());
    tokio::time::timeout_at(deadline, connect)
        .await
        .map_err(|_| sqlx::Error::PoolTimedOut)?
}
