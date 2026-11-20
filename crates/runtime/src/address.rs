use crate::Error;

/// The durable address of a lane: surface, conversation, and lane id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub(crate) surface: String,
    pub(crate) conversation: String,
    pub(crate) lane: String,
}

impl Address {
    /// # Errors
    /// Returns `invalid_input` when any external identity is empty.
    pub fn new(surface: &str, conversation: &str, lane: &str) -> Result<Self, Error> {
        crate::prompt::require(surface, "surface")?;
        crate::prompt::require(conversation, "conversation")?;
        crate::prompt::require(lane, "lane")?;
        Ok(Self {
            surface: surface.to_owned(),
            conversation: conversation.to_owned(),
            lane: lane.to_owned(),
        })
    }

    #[must_use]
    /// The surface name the prompt arrived on.
    pub fn surface(&self) -> &str {
        &self.surface
    }

    #[must_use]
    /// The surface's id for the conversation.
    pub fn conversation(&self) -> &str {
        &self.conversation
    }

    #[must_use]
    /// The lane's id within the conversation.
    pub fn lane(&self) -> &str {
        &self.lane
    }
}
