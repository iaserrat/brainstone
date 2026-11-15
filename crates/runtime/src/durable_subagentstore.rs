use crate::{DurableStore, Error, OperationId, SpawnedSubagent, Subagent, SubagentId, SubagentRequest, SubagentStore};
use crate::durable::dispatch;

impl SubagentStore for DurableStore {
    async fn spawn_subagent(&self, request: &SubagentRequest) -> Result<SpawnedSubagent, Error> {
        dispatch!(self, spawn_subagent(request))
    }

    async fn subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
    ) -> Result<Option<Subagent>, Error> {
        dispatch!(self, subagent(caller, id))
    }

    async fn message_subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
        key: &str,
        message: &str,
    ) -> Result<Subagent, Error> {
        dispatch!(self, message_subagent(caller, id, key, message))
    }

    async fn stop_subagent(&self, caller: OperationId, id: SubagentId) -> Result<Subagent, Error> {
        dispatch!(self, stop_subagent(caller, id))
    }

}
