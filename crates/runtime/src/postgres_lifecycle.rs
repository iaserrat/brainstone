use crate::{Error, PostgresStore};

mod migration_recovery;

impl PostgresStore {
    /// Connect, require durable commits, and apply embedded migrations.
    ///
    /// # Errors
    /// Returns `store_unavailable` when setup cannot complete.
    pub async fn connect(url: &str) -> Result<Self, Error> {
        let pool = crate::postgres_pool::options()
            .connect(url)
            .await
            .map_err(Error::store)?;
        crate::postgres_durability::verify(&pool).await?;
        migration_recovery::migrate(&pool).await?;
        Ok(Self { pool })
    }

    /// # Errors
    /// Returns `store_unavailable` when no connection can execute a query.
    pub async fn ping(&self) -> Result<(), Error> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(Error::store)?;
        Ok(())
    }

/// Closes the pool, waiting for in-flight statements.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}
