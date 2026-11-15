use super::Runtime;
use crate::{
    Error, OperationId, SpawnedSubagent, Store, Subagent, SubagentId, SubagentRequest,
    SubagentStore,
};

impl<S: Store + SubagentStore> Runtime<S> {
    /// Atomically create or reopen one child for a durable spawn key.
    ///
    /// # Errors
    /// Returns a stable store or idempotency error.
    pub async fn spawn_subagent(
        &self,
        request: &SubagentRequest,
    ) -> Result<SpawnedSubagent, Error> {
        let span = agent::observation::span(agent::observation::Kind::Subagent);
        span.record("parent_operation_id", request.parent.get());
        let result = agent::observation::run(
            span.clone(),
            self.store.spawn_subagent(request),
            agent::observation::result_outcome,
        )
        .await;
        if let Ok(spawned) = &result {
            span.record("child_operation_id", spawned.agent.operation.id.get());
        }
        result
    }

    /// Read the latest durable operation for a child.
    ///
    /// # Errors
    /// Rejects a child outside the caller's tree.
    pub async fn subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
    ) -> Result<Option<Subagent>, Error> {
        self.store.subagent(caller, id).await
    }

    /// Queue a durable follow-up in the child's existing context.
    ///
    /// # Errors
    /// Returns a stable store, validation, or authorization error.
    pub async fn message_subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
        key: &str,
        message: &str,
    ) -> Result<Subagent, Error> {
        self.store.message_subagent(caller, id, key, message).await
    }

    /// Fence and fail the child's current operation.
    ///
    /// # Errors
    /// Rejects a child outside the caller's tree.
    pub async fn stop_subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
    ) -> Result<Subagent, Error> {
        self.store.stop_subagent(caller, id).await
    }
}
