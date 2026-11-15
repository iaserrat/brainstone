use serde::Serialize;

/// Controls how many tokens the model spends on reasoning.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    /// No reasoning before the answer.
    None,
    /// The least reasoning the target accepts.
    Minimal,
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

impl From<agent::Effort> for Effort {
    fn from(value: agent::Effort) -> Self {
        match value {
            agent::Effort::Low => Self::Low,
            agent::Effort::Medium => Self::Medium,
            agent::Effort::High => Self::High,
            agent::Effort::XHigh => Self::XHigh,
            agent::Effort::Max => Self::Max,
        }
    }
}
