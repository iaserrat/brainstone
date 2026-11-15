//! Authenticated surface facts. Only trusted ingress code may construct these.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// The authenticated facts a surface supplies. Only trusted ingress may build
/// these.
pub struct ExecutionIdentity {
    /// The surface's own name.
    pub provider: String,
    /// The tenant the request belongs to.
    pub tenant: String,
    /// The authenticated principal that sent it.
    pub actor: String,
    /// The surface's id for the conversation.
    pub conversation: String,
    /// Whether this conversation is readable by this principal alone. A
    /// surface that does not model visibility answers `false`: personal-data
    /// features stay off, and the conversation counts as shared.
    #[serde(default)]
    pub private: bool,
}

impl ExecutionIdentity {
    #[must_use]
    /// Whether every required field is non-blank.
    pub fn is_valid(&self) -> bool {
        [
            &self.provider,
            &self.tenant,
            &self.actor,
            &self.conversation,
        ]
        .iter()
        .all(|value| !value.trim().is_empty())
    }

    /// Collision-free tenant scope, also used by durable memory keys.
    #[must_use]
    pub fn scope(&self) -> String {
        serde_json::json!([self.provider, self.tenant]).to_string()
    }

    /// Private conversations also bind their owner; shared conversations remain shared.
    #[must_use]
    pub fn conversation_key(&self) -> String {
        let owner = self.private.then_some(&self.actor);
        serde_json::json!([self.provider, self.tenant, self.conversation, owner]).to_string()
    }
}
