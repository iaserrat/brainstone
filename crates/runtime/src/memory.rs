use std::sync::{Arc, Mutex};

use crate::memory_state::State;
use std::time::Duration;

use agent::Event;
use serde_json::Value;

use crate::{
    Accepted, Admitted, Claimed, Compaction, Error, Lease, Operation, OperationId, Prompt, Settled,
    Store,
};

/// Process-local store for deterministic contracts. It provides no durability.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    pub(crate) state: Arc<Mutex<State>>,
}

#[expect(clippy::unused_async_trait_impl, reason = "Preserve execution on poll across the async trait seam.")]
impl Store for MemoryStore {
    async fn accept(&self, prompt: &Prompt) -> Result<Accepted, Error> {
        let mut state = self.state.lock().expect("memory store poisoned");
        state.accept(prompt, false)
    }

    /// The memory store keeps no tool definitions: claims build the list.
    async fn admit_with_tools(
        &self,
        prompt: &Prompt,
        _definitions: &[Value],
    ) -> Result<Admitted, Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .admit(prompt)
    }

    async fn operation(&self, id: OperationId) -> Result<Option<Operation>, Error> {
        let state = self.state.lock().expect("memory store poisoned");
        Ok(state.operation(id))
    }

    async fn claim(&self, worker: &str, ttl: Duration) -> Result<Option<Claimed>, Error> {
        let mut state = self.state.lock().expect("memory store poisoned");
        Ok(state.claim(worker, ttl))
    }

    async fn claim_operation(
        &self,
        id: OperationId,
        worker: &str,
        ttl: Duration,
    ) -> Result<Option<Claimed>, Error> {
        let mut state = self.state.lock().expect("memory store poisoned");
        Ok(state.claim_operation(id, worker, ttl))
    }

    async fn record(&self, lease: &Lease, event: &Event) -> Result<(), Error> {
        let mut state = self.state.lock().expect("memory store poisoned");
        state.record(lease, event)
    }

    async fn save_media(&self, lease: &Lease, media: &[crate::Media]) -> Result<(), Error> {
        let mut state = self.state.lock().expect("memory store poisoned");
        state.save_media(lease, media)
    }

    async fn steering(&self, lease: &Lease) -> Result<Vec<agent::Message>, Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .steering(lease)
    }

    async fn has_steering(&self, lease: &Lease) -> Result<Option<bool>, Error> {
        let state = self.state.lock().expect("memory store poisoned");
        state.runs.has_steering(lease, &state.steerings).map(Some)
    }

    async fn settle(&self, lease: &Lease, outcome: &agent::Outcome) -> Result<Settled, Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .settle(lease, outcome)
    }

    async fn heartbeat(&self, lease: &Lease, ttl: Duration) -> Result<(), Error> {
        let mut state = self.state.lock().expect("memory store poisoned");
        state.heartbeat(lease, ttl)
    }

    async fn compact(&self, compaction: &Compaction) -> Result<(), Error> {
        let mut state = self.state.lock().expect("memory store poisoned");
        state.compact(compaction)
    }
}
