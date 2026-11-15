use crate::{AdmissionQuota, Error, PostgresStore};

impl AdmissionQuota for PostgresStore {
    async fn actor_operations_last_day(&self, actor: &str) -> Result<usize, Error> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM operations \
             WHERE actor = $1 AND created_at > now() - INTERVAL '1 day'",
        )
        .bind(actor)
        .fetch_one(&self.pool)
        .await
        .map_err(Error::store)?;
        Ok(usize::try_from(count).unwrap_or(0))
    }
}
