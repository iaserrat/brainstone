use crate::{
    Error, OperationId, PostgresStore, SpawnedSubagent, Subagent, SubagentId, SubagentRequest,
    SubagentStore,
};

impl SubagentStore for PostgresStore {
    async fn spawn_subagent(&self, request: &SubagentRequest) -> Result<SpawnedSubagent, Error> {
        crate::postgres_subagent::spawn(&self.pool, request).await
    }

    async fn subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
    ) -> Result<Option<Subagent>, Error> {
        crate::postgres_subagent_read::get(&self.pool, caller, id).await
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
        crate::postgres_subagent_stop::stop(&self.pool, caller, id).await
    }
}
