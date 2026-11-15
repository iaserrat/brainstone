use crate::{
    Error, MemoryStore, OperationId, SpawnedSubagent, Subagent, SubagentId, SubagentRequest,
    SubagentStore,
};

#[expect(clippy::unused_async_trait_impl, reason = "Preserve execution on poll across the async trait seam.")]
impl SubagentStore for MemoryStore {
    async fn spawn_subagent(&self, request: &SubagentRequest) -> Result<SpawnedSubagent, Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .spawn_subagent(request)
    }

    async fn subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
    ) -> Result<Option<Subagent>, Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .subagent(caller, id)
    }

    async fn message_subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
        key: &str,
        message: &str,
    ) -> Result<Subagent, Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .message_subagent(caller, id, key, message)
    }

    async fn stop_subagent(&self, caller: OperationId, id: SubagentId) -> Result<Subagent, Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .stop_subagent(caller, id)
    }
}
