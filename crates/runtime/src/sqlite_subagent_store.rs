use crate::{
    Error, OperationId, SqliteStore, SpawnedSubagent, Subagent, SubagentId, SubagentRequest,
    SubagentStore,
};

impl SubagentStore for SqliteStore {
    async fn spawn_subagent(&self, request: &SubagentRequest) -> Result<SpawnedSubagent, Error> {
        crate::sqlite_subagent::spawn(&self.pool, request).await
    }

    async fn subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
    ) -> Result<Option<Subagent>, Error> {
        crate::sqlite_subagent_read::get(&self.pool, caller, id).await
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
        crate::sqlite_subagent_stop::stop(&self.pool, caller, id).await
    }
}
