use crate::DeliveryLease;

/// How far a target has been delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryCheckpoint {
    pub(crate) lease: DeliveryLease,
    pub(crate) through: Option<i64>,
    pub(crate) external_id: Option<String>,
}

impl DeliveryCheckpoint {
    #[must_use]
    /// A checkpoint for `lease` through `through`.
    pub const fn new(lease: DeliveryLease, through: i64, external_id: Option<String>) -> Self {
        Self {
            lease,
            through: Some(through),
            external_id,
        }
    }

    /// Record that the surface is still presenting this live operation.
    #[must_use]
    pub const fn presented(lease: DeliveryLease) -> Self {
        Self {
            lease,
            through: None,
            external_id: None,
        }
    }
}
