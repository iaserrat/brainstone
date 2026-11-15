use crate::memory::MemoryStore;
use crate::{AdmissionQuota, Error};

#[expect(clippy::unused_async_trait_impl, reason = "Preserve execution on poll across the async trait seam.")]
impl AdmissionQuota for MemoryStore {
    /// A process-local store holds nothing older than the process itself, so
    /// every registered operation counts as within the trailing day.
    async fn actor_operations_last_day(&self, actor: &str) -> Result<usize, Error> {
        let state = self.state.lock().expect("memory store poisoned");
        Ok(state.runs.actor_operations(actor))
    }
}
