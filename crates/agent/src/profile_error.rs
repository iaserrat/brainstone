use std::{error, fmt};

/// Why an execution profile was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProfileError {
    /// The model id is blank.
    EmptyModel,
    /// The response limit is zero.
    InvalidMaxTurns,
}

impl ProfileError {
    #[must_use]
    /// The stable code a caller branches on.
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyModel => "empty_model",
            Self::InvalidMaxTurns => "invalid_max_turns",
        }
    }
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyModel => "model is empty",
            Self::InvalidMaxTurns => "max turns must be positive",
        })
    }
}

impl error::Error for ProfileError {}
