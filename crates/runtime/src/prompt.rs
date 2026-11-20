use agent::ExecutionProfile;

use crate::{Address, DeliveryTarget, Error, PromptContent};

#[path = "prompt_builders.rs"]
mod builders;

/// One prompt: where it goes, its retry key, its system prompt, and its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub(crate) address: Address,
    pub(crate) request_id: String,
    pub(crate) system: String,
    pub(crate) content: PromptContent,
    pub(crate) source_text: String,
    pub(crate) delivery: Option<DeliveryTarget>,
    pub(crate) profile: ExecutionProfile,
    pub(crate) actor: Option<String>,
    pub(crate) identity: Option<agent::ExecutionIdentity>,
}

impl Prompt {
    /// A text-only prompt.
    ///
    /// # Errors
    ///
    /// Returns `invalid_input` when the request identity or prompt is empty.
    pub fn new(
        address: Address,
        request_id: &str,
        system: &str,
        text: &str,
    ) -> Result<Self, Error> {
        Self::with_content(
            address,
            request_id,
            system,
            PromptContent {
                text: text.to_owned(),
                ..PromptContent::default()
            },
        )
    }

    /// A prompt carrying whatever the user turn holds: text and images,
    /// in any combination that is not entirely empty.
    ///
    /// # Errors
    ///
    /// Returns `invalid_input` when the request identity or content is empty.
    pub fn with_content(
        address: Address,
        request_id: &str,
        system: &str,
        content: PromptContent,
    ) -> Result<Self, Error> {
        require(request_id, "request_id")?;
        if content.is_empty() {
            return Err(Error::InvalidInput { field: "content" });
        }
        let source_text = content.text.clone();
        Ok(Self {
            address,
            request_id: request_id.to_owned(),
            system: system.to_owned(),
            content,
            source_text,
            delivery: None,
            profile: ExecutionProfile::default(),
            actor: None,
            identity: None,
        })
    }
}

pub(crate) fn require(value: &str, field: &'static str) -> Result<(), Error> {
    if value.is_empty() {
        return Err(Error::InvalidInput { field });
    }
    Ok(())
}
