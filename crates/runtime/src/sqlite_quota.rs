use crate::{AdmissionQuota, Error, SqliteStore};

impl AdmissionQuota for SqliteStore {
    async fn actor_operations_last_day(&self, actor: &str) -> Result<usize, Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM operations \
             WHERE actor = ?1 AND created_at > CAST(unixepoch('subsec') * 1000 AS INTEGER) - 86400000",
        )
        .bind(actor)
        .fetch_one(&self.pool)
        .await
        .map_err(crate::sqlite_error::store)?;
        Ok(usize::try_from(count).unwrap_or(0))
    }
}
