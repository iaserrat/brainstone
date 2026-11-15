use crate::memory::MemoryStore;
use crate::{Error, Memories, Memory, MemoryScope, OperationId};

#[expect(clippy::unused_async_trait_impl, reason = "Preserve execution on poll across the async trait seam.")]
impl Memories for MemoryStore {
    async fn save_memory(&self, memory: &Memory) -> Result<(), Error> {
        let mut state = self.state.lock().expect("memory store poisoned");
        if let Some(stored) = state
            .memories
            .iter()
            .find(|stored| stored.key == memory.key)
        {
            if stored == memory {
                return Ok(());
            }
            return Err(Error::IdempotencyConflict);
        }
        state.memories.push(memory.clone());
        Ok(())
    }

    async fn recall_memories(
        &self,
        scope: &str,
        filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, Error> {
        let state = self.state.lock().expect("memory store poisoned");
        Ok(state
            .memories
            .iter()
            .rev()
            .filter(|memory| memory.scope == scope && matches(memory, filter))
            .take(limit)
            .map(|memory| memory.content.clone())
            .collect())
    }

    async fn memory_scope(&self, operation: OperationId) -> Result<MemoryScope, Error> {
        let state = self.state.lock().expect("memory store poisoned");
        let prompt = state.runs.prompt(operation).ok_or(Error::InvalidInput {
            field: "operation_id",
        })?;
        Ok(prompt.memory_scope())
    }
}

fn matches(memory: &Memory, filter: Option<&str>) -> bool {
    filter.is_none_or(|filter| {
        memory
            .content
            .to_lowercase()
            .contains(&filter.to_lowercase())
    })
}
