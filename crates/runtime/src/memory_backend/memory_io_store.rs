use serde_json::Value;

use crate::{
    Error, Interaction, InteractionRequest, InteractionStore, MemoryStore, OperationId, ScheduleId,
    ScheduleRequest, ScheduleStore, Scheduled,
};

#[path = "memory_delivery_store.rs"]
mod delivery;

#[expect(
    clippy::unused_async_trait_impl,
    reason = "Preserve execution on poll across the async trait seam."
)]
impl ScheduleStore for MemoryStore {
    async fn schedule(&self, request: &ScheduleRequest) -> Result<Scheduled, Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .schedule(request)
    }

    async fn schedules(&self, operation_id: OperationId) -> Result<Vec<Scheduled>, Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .schedules(operation_id)
    }

    async fn cancel_schedule(
        &self,
        operation_id: OperationId,
        schedule_id: ScheduleId,
    ) -> Result<Scheduled, Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .cancel_schedule(operation_id, schedule_id)
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "Preserve execution on poll across the async trait seam."
)]
impl InteractionStore for MemoryStore {
    async fn open_interaction(&self, request: &InteractionRequest) -> Result<Interaction, Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .open_interaction(request)
    }

    async fn mark_interaction_delivered(&self, id: &str, ts: &str) -> Result<(), Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .deliver_interaction(id, ts)
    }

    async fn answer_interaction(&self, id: &str, response: &Value) -> Result<(), Error> {
        self.state
            .lock()
            .expect("memory store poisoned")
            .answer_interaction(id, response)
    }

    async fn interaction(&self, id: &str) -> Result<Option<Interaction>, Error> {
        Ok(self
            .state
            .lock()
            .expect("memory store poisoned")
            .interaction(id))
    }
}
