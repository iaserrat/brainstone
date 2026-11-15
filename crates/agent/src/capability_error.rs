use crate::{ModelResponse, StopReason};

/// A request requires a feature the configured model does not provide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilityError {
    /// The request offers tools the model cannot call.
    Tools,
    /// The thread carries images the model cannot read.
    Images,
    /// A tool returned an image the model cannot read.
    ToolImages,
    /// The profile asks for a reasoning effort the model does not support.
    ReasoningEffort,
}

impl CapabilityError {
    #[must_use]
    /// The stable code a caller branches on.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Tools => "unsupported_tools",
            Self::Images => "unsupported_images",
            Self::ToolImages => "unsupported_tool_images",
            Self::ReasoningEffort => "unsupported_reasoning_effort",
        }
    }

    pub(crate) fn response(self) -> ModelResponse {
        ModelResponse {
            model: None,
            text: String::new(),
            reasoning: Vec::new(),
            tool_calls: Vec::new(),
            stop: StopReason::Error {
                code: self.code().to_owned(),
                message: self.to_string(),
            },
        }
    }
}

impl std::fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "model capability unavailable: {}", self.code())
    }
}
impl std::error::Error for CapabilityError {}
