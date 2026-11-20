//! One turn-limit contract for execution profiles and configuration.
use std::{num::NonZeroUsize, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ProfileError;

/// Unlimited execution or an explicitly selected positive response count.
/// The wire form is a positive integer or the string `"unlimited"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Value")]
pub enum TurnLimit {
    #[default]
    /// No response limit.
    Unlimited,
    /// A positive response limit.
    Limited(NonZeroUsize),
}

impl TurnLimit {
    /// The configured response count; `None` means unlimited.
    #[must_use]
    pub const fn get(self) -> Option<usize> {
        match self {
            Self::Unlimited => None,
            Self::Limited(value) => Some(value.get()),
        }
    }
}

impl TryFrom<Option<usize>> for TurnLimit {
    type Error = ProfileError;

    fn try_from(value: Option<usize>) -> Result<Self, Self::Error> {
        match value {
            None => Ok(Self::Unlimited),
            Some(value) => NonZeroUsize::new(value)
                .map(Self::Limited)
                .ok_or(ProfileError::InvalidMaxTurns),
        }
    }
}

impl FromStr for TurnLimit {
    type Err = ProfileError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "unlimited" {
            return Ok(Self::Unlimited);
        }
        value
            .parse()
            .map(Self::Limited)
            .map_err(|_| ProfileError::InvalidMaxTurns)
    }
}

impl TryFrom<Value> for TurnLimit {
    type Error = ProfileError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(value) if value == "unlimited" => Ok(Self::Unlimited),
            Value::Number(value) => value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .and_then(NonZeroUsize::new)
                .map(Self::Limited)
                .ok_or(ProfileError::InvalidMaxTurns),
            _ => Err(ProfileError::InvalidMaxTurns),
        }
    }
}

impl From<TurnLimit> for Value {
    fn from(value: TurnLimit) -> Self {
        match value {
            TurnLimit::Unlimited => Self::String("unlimited".to_owned()),
            TurnLimit::Limited(value) => Self::from(value.get()),
        }
    }
}
