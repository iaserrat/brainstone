/// An expected tool failure, reported to the model rather than the operator.
/// `code` is the stable slug the model branches on; `message` is detail.
/// Tool authors own this open namespace: map typed implementation errors to a
/// documented stable code here rather than requiring a central framework enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    /// The stable slug the model branches on.
    pub code: &'static str,
    /// Detail for humans; never the branching signal.
    pub message: String,
}

impl ToolError {
    #[must_use]
    /// Builds an error from a stable code and detail.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ToolError {}
