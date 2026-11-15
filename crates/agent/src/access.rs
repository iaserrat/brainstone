use crate::{ExecutionIdentity, ToolError};
use serde::{Deserialize, Serialize};

/// Operator-owned requirements, never taken from a model or server tool schema.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessPolicy {
    #[serde(default)]
    /// Require an authenticated, non-blank actor identity.
    pub authenticated_actor: bool,
    #[serde(default)]
    /// Require a private conversation: readable by this principal alone.
    pub private_conversation: bool,
}

/// Why a policy check refused the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccessError {
    /// No authenticated actor identity was supplied.
    AuthenticatedActorRequired,
    /// The conversation is not private to the caller.
    PrivateConversationRequired,
}

impl AccessPolicy {
    /// # Errors
    /// Fails closed when required authenticated facts are absent or invalid.
    pub fn check(self, identity: Option<&ExecutionIdentity>) -> Result<(), AccessError> {
        if !self.authenticated_actor && !self.private_conversation {
            return Ok(());
        }
        let identity = identity
            .filter(|identity| identity.is_valid())
            .ok_or(AccessError::AuthenticatedActorRequired)?;
        if self.private_conversation && !identity.private {
            return Err(AccessError::PrivateConversationRequired);
        }
        Ok(())
    }
}

impl AccessError {
    #[must_use]
    /// The stable code a caller branches on.
    pub const fn code(self) -> &'static str {
        match self {
            Self::AuthenticatedActorRequired => "authenticated_actor_required",
            Self::PrivateConversationRequired => "private_conversation_required",
        }
    }
}

impl From<AccessError> for ToolError {
    fn from(error: AccessError) -> Self {
        Self::new(
            error.code(),
            "execution context does not satisfy tool access policy",
        )
    }
}
