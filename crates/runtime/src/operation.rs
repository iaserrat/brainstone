use serde::{Deserialize, Serialize};

use agent::Outcome;

/// The store's id for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationId(pub(crate) i64);

impl OperationId {
    /// # Errors
    ///
    /// Returns `invalid_input` unless the stored identifier is positive.
    pub fn new(value: i64) -> Result<Self, crate::Error> {
        (value > 0)
            .then_some(Self(value))
            .ok_or(crate::Error::InvalidInput {
                field: "operation_id",
            })
    }

    #[must_use]
    /// The raw id.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// The store's id for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub(crate) i64);

impl SessionId {
    #[must_use]
    /// The raw id.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// The store's id for a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LaneId(pub(crate) i64);

impl LaneId {
    #[must_use]
    /// The raw id.
    pub const fn get(self) -> i64 {
        self.0
    }
}

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
    /// Finished without one.
    Failed,
}

/// One durable operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
