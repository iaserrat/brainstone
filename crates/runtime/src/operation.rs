use serde::{Deserialize, Serialize};

use agent::Outcome;

#[path = "operation_validation.rs"]
pub(crate) mod validation;
use validation::RawOperation;

#[path = "operation_ids.rs"]
mod ids;
pub use ids::{LaneId, OperationId, SessionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Where a durable operation stands.
pub enum OperationState {
    /// Stored, not yet claimed.
    Accepted,
    /// Claimed by a worker and running.
    Running,
    /// Finished with an outcome.
    Completed,
    /// Finished with a failed outcome.
    Failed,
}

/// One durable operation.
///
/// Deserialization rejects a state that disagrees with its outcome. Public
/// fields support custom stores; call [`Operation::validate`] after construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawOperation")]
pub struct Operation {
    /// Its id.
    pub id: OperationId,
    /// The session it belongs to.
    pub session_id: SessionId,
    /// The lane it runs in.
    pub lane_id: LaneId,
    /// Where it stands.
    pub state: OperationState,
    /// The settled outcome, once finished.
    pub outcome: Option<Outcome>,
}

/// A newly accepted operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accepted {
    /// The operation as stored.
    pub operation: Operation,
    /// Whether a durable retry had already stored it.
    pub duplicate: bool,
}

impl Accepted {
    pub(crate) const fn fresh(operation: Operation) -> Self {
        Self {
            operation,
            duplicate: false,
        }
    }
}
