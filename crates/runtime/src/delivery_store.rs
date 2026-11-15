use std::time::Duration;

use crate::{
    DeliveryBatch, DeliveryCheckpoint, DeliveryClaim, DeliveryLease, DeliveryTarget, Error,
    OperationId,
};

/// Durable delivery: claim pending events, renew the claim, and record progress.
pub trait DeliveryStore: Clone + Send + Sync {
    /// Claims the next batch for this target, or `None` when nothing is pending.
    fn claim_delivery(
        &self,
        claim: &DeliveryClaim,
    ) -> impl Future<Output = Result<Option<DeliveryBatch>, Error>> + Send;

    /// Records that every event through `checkpoint` was delivered.
    fn checkpoint_delivery(
        &self,
        checkpoint: &DeliveryCheckpoint,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Keep an in-flight external presentation fenced to this worker.
    fn renew_delivery(
        &self,
        lease: &DeliveryLease,
        ttl: Duration,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Persist opaque surface reconciliation state without advancing the
    /// cursor or releasing the lease. The legacy name remains API-compatible;
    /// adapters may store a versioned receipt containing more than one ID.
    fn save_delivery_stream(
        &self,
        lease: &DeliveryLease,
        external_id: Option<&str>,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Where an operation was accepted to deliver: its surface's own routing
    /// record, which tools use to find the thread they run in.
    fn delivery_target(
        &self,
        operation: OperationId,
    ) -> impl Future<Output = Result<Option<DeliveryTarget>, Error>> + Send;

    /// Name a post's lane after the conversation root the surface just
    /// created, and route the operation's remaining delivery to `target`, a
    /// reply there. `false` when another lane already holds that name: a
    /// message in the new thread arrived first. Only a lane a schedule opened
    /// for a post, still named by its request id, can be bound.
    fn bind_lane(
        &self,
        lease: &DeliveryLease,
        external_id: &str,
        target: &DeliveryTarget,
    ) -> impl Future<Output = Result<bool, Error>> + Send;

    /// The tool media `operation`'s reply names by `reference`, as any tool
    /// of its root conversation stored it: its own lanes and its subagents'.
    /// `None` for an unknown operation or reference, or media past retention.
    fn media(
        &self,
        operation: OperationId,
        reference: &str,
    ) -> impl Future<Output = Result<Option<crate::Media>, Error>> + Send;

    /// What a person wrote for this operation, before any context was added,
    /// for surface checks that must not trust model-visible text.
    fn source_text(
        &self,
        operation: OperationId,
    ) -> impl Future<Output = Result<Option<String>, Error>> + Send;
}
