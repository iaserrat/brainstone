use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// How much reasoning a turn may spend.
pub enum Effort {
    /// Little reasoning.
    Low,
    /// Moderate reasoning.
    Medium,
    /// More reasoning than `medium`.
    High,
    /// Extensive reasoning.
    XHigh,
    /// The most reasoning the model allows.
    Max,
}
