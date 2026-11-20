use crate::sqlite_backend::sqlite_error::store;
use crate::sqlite_backend::{sqlite_durability, sqlite_file, sqlite_migrations, sqlite_pool};
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};

use crate::{Error, SqliteStore};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

impl SqliteStore {
    /// Open a file-backed database, require durable WAL commits, and migrate it.
    /// All processes sharing the file must run on the same host.
    ///
    /// # Errors
    /// Rejects in-memory stores and unsafe settings; reports migration and I/O failures.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        let options = options(url)?;
        sqlite_file::create(options.get_filename())?;
        let pool = sqlite_pool::connect(options).await?;
        sqlite_durability::verify(&pool).await?;
        sqlite_migrations::migrate(&pool).await?;
        Ok(Self { pool })
    }

    /// # Errors
    /// Returns `store_unavailable` when the pool cannot execute a query.
    pub async fn ping(&self) -> Result<(), Error> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(store)?;
        Ok(())
    }

    // software-design-allow: pass-through-method exposes pool shutdown without exposing SQLx.
    /// Closes the pool, waiting for in-flight statements.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

fn options(url: &str) -> Result<SqliteConnectOptions, Error> {
    let options = SqliteConnectOptions::from_str(url).map_err(store)?;
    if options.get_filename().as_os_str().is_empty()
        || options.get_filename() == std::path::Path::new(":memory:")
        || url.contains("mode=memory")
        || url.contains(":memory:")
    {
        return Err(Error::DurabilityNotGuaranteed {
            setting: "sqlite_storage",
            value: "memory".to_owned(),
        });
    }
    Ok(options
        .create_if_missing(false)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full)
        .busy_timeout(BUSY_TIMEOUT))
}
