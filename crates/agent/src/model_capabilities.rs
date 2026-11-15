use crate::{Message, Thread};

/// Features promised by a configured model, independent of its wire protocol.
/// Defaults preserve the unrestricted contract of existing `Model` implementations.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent endpoint features, not mutually exclusive states"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelCapabilities {
    /// The endpoint accepts tool definitions and calls.
    pub tools: bool,
    /// The endpoint accepts user images.
    pub images: bool,
    /// The endpoint accepts images returned by tools.
    pub tool_images: bool,
    /// The endpoint accepts a reasoning effort.
    pub reasoning_effort: bool,
    /// The endpoint streams progress instead of answering whole.
    pub streaming: bool,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            tools: true,
            images: true,
            tool_images: true,
            reasoning_effort: true,
            streaming: true,
        }
    }
}

impl ModelCapabilities {
    /// # Errors
    /// Names the capability required by the request that this model cannot honor.
    pub fn validate(&self, thread: &Thread, has_tools: bool) -> Result<(), crate::CapabilityError> {
        use crate::CapabilityError::{ReasoningEffort, Tools};
        require(self.tools || !has_tools, Tools)?;
        require(
            self.reasoning_effort || thread.profile().effort().is_none(),
            ReasoningEffort,
        )?;
        for message in thread.messages() {
            self.message(message)?;
        }
        Ok(())
    }

    fn message(self, message: &Message) -> Result<(), crate::CapabilityError> {
        use crate::CapabilityError::{Images, ToolImages, Tools};
        match message {
            Message::UserImages(_) => require(self.images, Images),
            Message::Assistant(response) => {
                require(self.tools || response.tool_calls.is_empty(), Tools)
            }
            Message::ToolResults(results) => require(
                self.tool_images || results.iter().all(|result| result.images.is_empty()),
                ToolImages,
            ),
            _ => Ok(()),
        }
    }
}

fn require(supported: bool, failure: crate::CapabilityError) -> Result<(), crate::CapabilityError> {
    supported.then_some(()).ok_or(failure)
}
