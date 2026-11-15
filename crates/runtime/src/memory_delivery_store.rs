use std::time::Duration;

use crate::{
    DeliveryBatch, DeliveryCheckpoint, DeliveryClaim, DeliveryLease, DeliveryStore,
    DeliveryTarget, Error, MemoryStore, OperationId,
};

#[expect(clippy::unused_async_trait_impl, reason = "Preserve execution on poll across the async trait seam.")]
impl DeliveryStore for MemoryStore {
    async fn claim_delivery(&self, claim: &DeliveryClaim) -> Result<Option<DeliveryBatch>, Error> {
        self.state().claim_delivery(claim)
    }

    async fn checkpoint_delivery(&self, checkpoint: &DeliveryCheckpoint) -> Result<(), Error> {
        self.state().checkpoint_delivery(checkpoint)
    }

    async fn renew_delivery(&self, lease: &DeliveryLease, ttl: Duration) -> Result<(), Error> {
        self.state().deliveries.renew(lease, ttl)
    }

    async fn save_delivery_stream(
        &self,
        lease: &DeliveryLease,
        external_id: Option<&str>,
    ) -> Result<(), Error> {
        self.state().save_delivery_stream(lease, external_id)
    }

    async fn delivery_target(&self, operation: OperationId) -> Result<Option<DeliveryTarget>, Error> {
        let rows = &self.state().deliveries.rows;
        Ok(rows.get(&operation).map(|row| row.target.clone()))
    }

    async fn bind_lane(
        &self,
        lease: &DeliveryLease,
        external_id: &str,
        target: &DeliveryTarget,
    ) -> Result<bool, Error> {
        self.state().bind_lane(lease, external_id, target)
    }

    async fn media(
        &self,
        operation: OperationId,
        reference: &str,
    ) -> Result<Option<crate::Media>, Error> {
        Ok(self.state().media(operation, reference))
    }

    async fn source_text(&self, operation: OperationId) -> Result<Option<String>, Error> {
        let state = self.state();
        Ok(state.runs.prompt(operation).map(|prompt| prompt.source_text.clone()))
    }
}

impl MemoryStore {
    fn state(&self) -> std::sync::MutexGuard<'_, crate::memory_state::State> {
        self.state.lock().expect("memory store poisoned")
    }
}
