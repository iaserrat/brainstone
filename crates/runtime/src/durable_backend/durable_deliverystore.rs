use crate::durable_backend::durable::dispatch;
use crate::{
    DeliveryBatch, DeliveryCheckpoint, DeliveryClaim, DeliveryLease, DeliveryStore, DeliveryTarget,
    DurableStore, Error, OperationId,
};
use std::time::Duration;

impl DeliveryStore for DurableStore {
    async fn claim_delivery(&self, claim: &DeliveryClaim) -> Result<Option<DeliveryBatch>, Error> {
        dispatch!(self, claim_delivery(claim))
    }

    async fn checkpoint_delivery(&self, checkpoint: &DeliveryCheckpoint) -> Result<(), Error> {
        dispatch!(self, checkpoint_delivery(checkpoint))
    }

    async fn renew_delivery(&self, lease: &DeliveryLease, ttl: Duration) -> Result<(), Error> {
        dispatch!(self, renew_delivery(lease, ttl))
    }

    async fn save_delivery_stream(
        &self,
        lease: &DeliveryLease,
        external_id: Option<&str>,
    ) -> Result<(), Error> {
        dispatch!(self, save_delivery_stream(lease, external_id))
    }

    async fn delivery_target(
        &self,
        operation: OperationId,
    ) -> Result<Option<DeliveryTarget>, Error> {
        dispatch!(self, delivery_target(operation))
    }

    async fn bind_lane(
        &self,
        lease: &DeliveryLease,
        external_id: &str,
        target: &DeliveryTarget,
    ) -> Result<bool, Error> {
        dispatch!(self, bind_lane(lease, external_id, target))
    }

    async fn media(
        &self,
        operation: OperationId,
        reference: &str,
    ) -> Result<Option<crate::Media>, Error> {
        dispatch!(self, media(operation, reference))
    }

    async fn source_text(&self, operation: OperationId) -> Result<Option<String>, Error> {
        dispatch!(self, source_text(operation))
    }
}
