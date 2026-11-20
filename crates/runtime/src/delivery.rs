//! Surface delivery: fenced journal batches and live presentation.

use agent::Event;

use crate::{DeliveryTarget, OperationId};

#[path = "delivery_checkpoint.rs"]
mod checkpoint;
#[path = "delivery_claim.rs"]
mod claim;
#[path = "delivery_store.rs"]
mod store;

pub use checkpoint::DeliveryCheckpoint;
pub use claim::DeliveryClaim;
pub use store::DeliveryStore;

/// One committed event awaiting delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryEvent {
    /// The lane's sequence number for the event.
    pub sequence: i64,
    /// The event itself.
    pub event: Event,
}

/// A surface's exclusive claim on a lane's pending deliveries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryLease {
    pub(crate) operation_id: OperationId,
    pub(crate) worker: String,
    pub(crate) token: i64,
}

impl DeliveryLease {
    /// The operation this delivery speaks for. An adapter stamps it on what it
    /// posts so a retry can recognise its own prior work rather than repeat it.
    #[must_use]
    pub const fn operation(&self) -> OperationId {
        self.operation_id
    }
}

/// One claim's worth of deliveries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryBatch {
    /// The claim they were read under.
    pub lease: DeliveryLease,
    /// Where they go.
    pub target: DeliveryTarget,
    /// The surface's id for the posted message, when it already exists.
    pub external_id: Option<String>,
    /// The events to deliver, in sequence order.
    pub events: Vec<DeliveryEvent>,
    /// Runtime-owned text a surface appends to a failure it delivers, such as
    /// a standing schedule's failure streak. Never carries model output.
    pub notice: Option<String>,
    /// When the operation was accepted: nothing it presents is older.
    pub accepted_unix_ms: i64,
}
