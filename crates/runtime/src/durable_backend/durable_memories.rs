use crate::durable_backend::durable::dispatch;
use crate::{DurableStore, Error, Memories, Memory, MemoryScope, OperationId};

impl Memories for DurableStore {
    async fn save_memory(&self, memory: &Memory) -> Result<(), Error> {
        dispatch!(self, save_memory(memory))
    }

    async fn recall_memories(
        &self,
        scope: &str,
        filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, Error> {
        dispatch!(self, recall_memories(scope, filter, limit))
    }

    async fn memory_scope(&self, operation: OperationId) -> Result<MemoryScope, Error> {
        dispatch!(self, memory_scope(operation))
    }
}
