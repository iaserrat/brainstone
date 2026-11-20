use agent::{ExecutionProfile, Image, Message};

use crate::{Address, DeliveryTarget, Prompt};

/// Read accessors: what a surface or runner may learn about an accepted
/// prompt without changing it. Builders live with the struct in `prompt.rs`.
impl Prompt {
    #[must_use]
    /// The authenticated caller, when the surface supplied one.
    pub const fn identity(&self) -> Option<&agent::ExecutionIdentity> {
        self.identity.as_ref()
    }

    /// Lane configuration is checked separately from surface request identity.
    pub(crate) fn same_request(&self, other: &Self) -> bool {
        self.address == other.address
            && self.request_id == other.request_id
            && self.source_text == other.source_text
            && self.content.images == other.content.images
            && self.delivery == other.delivery
            && self.identity == other.identity
    }

    #[must_use]
    /// The profile this turn runs under.
    pub const fn profile(&self) -> &ExecutionProfile {
        &self.profile
    }

    #[must_use]
    /// Where the prompt goes.
    pub const fn address(&self) -> &Address {
        &self.address
    }

    #[must_use]
    /// The prompt's text.
    pub fn text(&self) -> &str {
        &self.content.text
    }

    #[must_use]
    /// Images sent with the prompt.
    pub fn images(&self) -> &[Image] {
        &self.content.images
    }

    pub(crate) fn message(&self) -> Message {
        Message::user(self.content.text.clone(), self.content.images.clone())
    }

    /// The durable request identity: the surface event this prompt came from.
    /// Idempotent acceptance keys on it, and surfaces key their own
    /// idempotent replies on it.
    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    #[must_use]
    /// Where the reply posts, when the prompt names a target.
    pub const fn delivery(&self) -> Option<&DeliveryTarget> {
        self.delivery.as_ref()
    }

    #[must_use]
    /// The surface user who sent it, when the surface supplied one.
    pub fn actor(&self) -> Option<&str> {
        self.actor.as_deref()
    }
}
