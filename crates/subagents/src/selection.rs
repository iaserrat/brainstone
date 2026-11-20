use serde::Deserialize;

/// Which model a child agent runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Model {
    /// Use the parent's model.
    Inherit,
    /// A provider alias such as `sonnet`, which the primary provider resolves.
    Alias(String),
    /// An exact model id.
    Id(String),
}

impl Model {
    pub(crate) fn parse(value: Option<String>) -> Self {
        match value.as_deref() {
            None | Some("inherit") => Self::Inherit,
            Some("sonnet" | "opus" | "haiku" | "fable") => {
                Self::Alias(value.expect("matched Some"))
            }
            Some(_) => Self::Id(value.expect("matched Some")),
        }
    }
}

/// How much reasoning a child spends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
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
