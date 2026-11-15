//! Refinements applied to an already-valid prompt.

use agent::ExecutionProfile;

use super::Prompt;
use crate::{DeliveryTarget, Error};

impl Prompt {
    /// Supply authenticated ingress facts. Model input must never reach this API.
    /// `None` deliberately grants no identity-bound authority.
    #[must_use]
    pub fn with_identity(mut self, identity: Option<agent::ExecutionIdentity>) -> Self {
        self.identity = identity;
        self
    }

    /// Record who asked: the surface-level user identity that authored this
    /// prompt, when the surface knows one.
    #[must_use]
    pub fn with_actor(mut self, actor: &str) -> Self {
        self.actor = Some(actor.to_owned());
        self
    }

    #[must_use]
    /// Posts the reply to `target` instead of replying in place.
    pub fn with_delivery(mut self, target: DeliveryTarget) -> Self {
        self.delivery = Some(target);
        self
    }

    #[must_use]
    /// Runs the turn under `profile`.
    pub fn with_profile(mut self, profile: ExecutionProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Replace model-visible user text while preserving the original surface
    /// text used for durable idempotency.
    ///
    /// # Errors
    ///
    /// Returns `invalid_input` when the replacement leaves the prompt empty.
    pub fn with_text(mut self, text: impl Into<String>) -> Result<Self, Error> {
        self.content.text = text.into();
        if self.content.is_empty() {
            return Err(Error::InvalidInput { field: "text" });
        }
        Ok(self)
    }
}
