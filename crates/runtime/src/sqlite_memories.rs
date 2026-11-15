use futures_util::TryStreamExt;
use crate::{Error, Memories, Memory, MemoryScope, OperationId, SqliteStore};

impl Memories for SqliteStore {
    async fn save_memory(&self, memory: &Memory) -> Result<(), Error> {
        let changed = sqlx::query(
            "INSERT INTO memories (scope, idempotency_key, content) VALUES (?1, ?2, ?3) \
             ON CONFLICT (idempotency_key) DO UPDATE \
             SET idempotency_key = EXCLUDED.idempotency_key \
             WHERE memories.scope = EXCLUDED.scope AND memories.content = EXCLUDED.content",
        )
        .bind(&memory.scope)
        .bind(&memory.key)
        .bind(&memory.content)
        .execute(&self.pool)
        .await
        .map_err(crate::sqlite_error::store)?;
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
        if limit == 0 { return Ok(Vec::new()); }
        let bound = if filter.is_some() { i64::MAX } else { i64::try_from(limit).unwrap_or(i64::MAX) };
        let mut rows = sqlx::query_scalar::<_, String>("SELECT content FROM memories WHERE scope=?1 ORDER BY id DESC LIMIT ?2")
            .bind(scope).bind(bound).fetch(&self.pool);
        let filter = filter.map(str::to_lowercase);
        let mut matches = Vec::new();
        while let Some(content) = rows.try_next().await.map_err(crate::sqlite_error::store)? {
            if filter.as_ref().is_none_or(|filter| content.to_lowercase().contains(filter)) { matches.push(content); }
            if matches.len() == limit { break; }
        }
        Ok(matches)
    }

    async fn memory_scope(&self, operation: OperationId) -> Result<MemoryScope, Error> {
        let mut connection = self.pool.acquire().await.map_err(crate::sqlite_error::store)?;
        if let Some(identity) = crate::sqlite_identity::load(&mut connection, operation).await? {
            return Ok(MemoryScope::authenticated(&identity));
        }
        let row: Option<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT sessions.surface, sessions.external_id, operations.actor FROM operations \
             JOIN lanes ON lanes.id = operations.lane_id \
             JOIN sessions ON sessions.id = lanes.session_id \
             WHERE operations.id = ?1",
        )
        .bind(operation.get())
        .fetch_optional(&mut *connection)
        .await
        .map_err(crate::sqlite_error::store)?;
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
