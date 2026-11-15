use crate::{Error, Memories, Memory, MemoryScope, OperationId, PostgresStore};

impl Memories for PostgresStore {
    async fn save_memory(&self, memory: &Memory) -> Result<(), Error> {
        let changed = sqlx::query(
            "INSERT INTO memories (scope, idempotency_key, content) VALUES ($1, $2, $3) \
             ON CONFLICT (idempotency_key) DO UPDATE \
             SET idempotency_key = EXCLUDED.idempotency_key \
             WHERE memories.scope = EXCLUDED.scope AND memories.content = EXCLUDED.content",
        )
        .bind(&memory.scope)
        .bind(&memory.key)
        .bind(&memory.content)
        .execute(&self.pool)
        .await
        .map_err(Error::store)?;
        if changed.rows_affected() != 1 {
            return Err(Error::IdempotencyConflict);
        }
        Ok(())
    }

    async fn recall_memories(
        &self,
        scope: &str,
        filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, Error> {
        sqlx::query_scalar(
            "SELECT content FROM memories \
             WHERE scope = $1 AND ($2::text IS NULL OR content ILIKE $2) \
             ORDER BY id DESC LIMIT $3",
        )
        .bind(scope)
        .bind(filter.map(pattern))
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await
        .map_err(Error::store)
    }

    async fn memory_scope(&self, operation: OperationId) -> Result<MemoryScope, Error> {
        let mut connection = self.pool.acquire().await.map_err(Error::store)?;
        if let Some(identity) = crate::postgres_identity::load(&mut connection, operation).await? {
            return Ok(MemoryScope::authenticated(&identity));
        }
        let row: Option<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT sessions.surface, sessions.external_id, operations.actor FROM operations \
             JOIN lanes ON lanes.id = operations.lane_id \
             JOIN sessions ON sessions.id = lanes.session_id \
             WHERE operations.id = $1",
        )
        .bind(operation.get())
        .fetch_optional(&mut *connection)
        .await
        .map_err(Error::store)?;
        let (surface, conversation, actor) = row.ok_or(Error::InvalidInput {
            field: "operation_id",
        })?;
        Ok(MemoryScope {
            surface,
            conversation,
            actor,
        })
    }
}

/// A literal-substring ILIKE pattern: LIKE metacharacters in the filter must
/// match themselves, so they are escaped before wrapping in wildcards.
fn pattern(filter: &str) -> String {
    let escaped = filter
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}
