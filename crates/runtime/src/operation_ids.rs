//! Positive store identifiers shared by operation records and custom backends.

use serde::{Deserialize, Serialize};

/// The store's id for a operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i64")]
pub struct OperationId(pub(crate) i64);

impl OperationId {
    /// Construct a positive identifier for a custom store.
    ///
    /// # Errors
    /// Returns `invalid_input` unless the identifier is positive.
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

impl TryFrom<i64> for OperationId {
    type Error = crate::Error;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// The store's id for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i64")]
pub struct SessionId(pub(crate) i64);

impl SessionId {
    /// Construct a positive identifier for a custom store.
    ///
    /// # Errors
    /// Returns `invalid_input` unless the identifier is positive.
    pub fn new(value: i64) -> Result<Self, crate::Error> {
        (value > 0)
            .then_some(Self(value))
            .ok_or(crate::Error::InvalidInput {
                field: "session_id",
            })
    }

    #[must_use]
    /// The raw id.
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for SessionId {
    type Error = crate::Error;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// The store's id for a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "i64")]
pub struct LaneId(pub(crate) i64);

impl LaneId {
    /// Construct a positive identifier for a custom store.
    ///
    /// # Errors
    /// Returns `invalid_input` unless the identifier is positive.
    pub fn new(value: i64) -> Result<Self, crate::Error> {
        (value > 0)
            .then_some(Self(value))
            .ok_or(crate::Error::InvalidInput { field: "lane_id" })
    }

    #[must_use]
    /// The raw id.
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for LaneId {
    type Error = crate::Error;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
