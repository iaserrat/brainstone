use crate::sqlite_backend::{sqlite_subagent, sqlite_subagent_read, sqlite_subagent_stop};
use crate::{
    Error, OperationId, SpawnedSubagent, SqliteStore, Subagent, SubagentId, SubagentRequest,
    SubagentStore,
};

impl SubagentStore for SqliteStore {
    async fn spawn_subagent(&self, request: &SubagentRequest) -> Result<SpawnedSubagent, Error> {
        sqlite_subagent::spawn(&self.pool, request).await
    }

    async fn subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
    ) -> Result<Option<Subagent>, Error> {
        sqlite_subagent_read::get(&self.pool, caller, id).await
    }

    async fn message_subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
        key: &str,
        message: &str,
    ) -> Result<Subagent, Error> {
        self.message(caller, id, key, message).await
    }

    async fn stop_subagent(&self, caller: OperationId, id: SubagentId) -> Result<Subagent, Error> {
        sqlite_subagent_stop::stop(&self.pool, caller, id).await
    }
}
