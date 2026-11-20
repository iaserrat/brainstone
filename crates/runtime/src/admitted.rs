use crate::{Accepted, Operation};

/// What durable admission produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admitted {
    /// A newly accepted operation.
    Operation(Accepted),
    /// A steering message accepted into a running operation.
    Steering {
        /// The operation it steers.
        operation: Operation,
        /// Whether a durable retry had already stored it.
        duplicate: bool,
    },
}

impl Admitted {
    #[must_use]
    /// The operation this admission produced.
    pub const fn operation(&self) -> &Operation {
        match self {
            Self::Operation(accepted) => &accepted.operation,
            Self::Steering { operation, .. } => operation,
        }
    }
}

impl From<Accepted> for Admitted {
    fn from(value: Accepted) -> Self {
        Self::Operation(value)
    }
}
