use crate::memory_backend::memory_state::State;
use crate::{DeliveryBatch, DeliveryCheckpoint, DeliveryClaim, Error};

#[path = "memory_delivery_checkpoint.rs"]
mod checkpoint;

impl State {
    pub(crate) fn claim_delivery(
        &mut self,
        claim: &DeliveryClaim,
    ) -> Result<Option<DeliveryBatch>, Error> {
        let batch = self.deliveries.claim(&self.operations, claim)?;
        Ok(batch.map(|mut batch| {
            let failures = self
                .schedules
                .consecutive_failures(batch.lease.operation_id);
            batch.notice = crate::schedule_failure::failure_notice(failures);
            batch
        }))
    }

    pub(crate) fn checkpoint_delivery(
        &mut self,
        checkpoint: &DeliveryCheckpoint,
    ) -> Result<(), Error> {
        self.deliveries.checkpoint(&self.operations, checkpoint)
    }

    pub(crate) fn save_delivery_stream(
        &mut self,
        lease: &crate::DeliveryLease,
        external_id: Option<&str>,
    ) -> Result<(), Error> {
        self.deliveries.save_stream(lease, external_id)
    }
}
