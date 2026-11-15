use crate::{Claimed, Error, OperationId};
use agent::{ExecutionIdentity, ToolCall};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// Where an approval request stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ApprovalState {
    /// The executor declares no approval requirement.
    Unsupported,
    /// Waiting for a decision.
    Pending,
    /// The decision allows this exact call.
    Approved,
    /// The decision refuses this exact call.
    Denied,
    /// The request is past its expiry and can no longer be approved.
    Expired,
}

/// Exact immutable authorization binding, constructed by the trusted runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRequest {
    /// The operation that asked.
    pub operation: OperationId,
    /// The idempotency key of the tool call.
    pub call_key: String,
    /// The exact call being approved.
    pub call: ToolCall,
    /// The tool definition the model saw.
    pub definition: Value,
    /// The authenticated caller, when the surface supplied one.
    pub identity: Option<ExecutionIdentity>,
    /// Digest over the call, identity, policy, and executor configuration.
    pub binding: String,
    version: u8,
}

impl ApprovalRequest {
    /// Bind the durable call, current definition, identity and execution policy.
    /// # Errors
    /// Rejects a call key outside the claimed operation or an empty policy binding.
    pub fn new(
        claim: &Claimed,
        call: (&str, &ToolCall, &Value),
        binding: &str,
    ) -> Result<Self, Error> {
        let prefix = format!("operation/{}/", claim.operation.id.get());
        if !call.0.starts_with(&prefix) || binding.is_empty() {
            return Err(Error::InvalidInput {
                field: "approval_binding",
            });
        }
        Ok(Self {
            operation: claim.operation.id,
            call_key: call.0.into(),
            call: call.1.clone(),
            definition: call.2.clone(),
            identity: claim.identity.clone(),
            binding: binding.into(),
            version: 1,
        })
    }

    pub(crate) fn known_version(&self) -> bool {
        self.version == 1
    }

    /// Stable ID of all bound data; a changed action or executor needs a new decision.
    /// # Panics
    /// Only if an in-memory JSON value cannot be serialized or written to a string.
    #[must_use]
    pub fn id(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("approval contains serializable JSON");
        Sha256::digest(bytes)
            .iter()
            .fold(String::new(), |mut text, byte| {
                write!(&mut text, "{byte:02x}").expect("writing string");
                text
            })
    }
}

/// One stored approval and its current state.
#[derive(Debug, Clone, Serialize)]
pub struct ApprovalRecord {
    /// The store's id for the request.
    pub id: String,
    /// The call and binding the decision applies to.
    pub request: ApprovalRequest,
    /// Where it stands.
    pub state: ApprovalState,
    /// Who decided, once a decision exists.
    pub actor: Option<String>,
    /// When the request stops being answerable.
    pub expires_at_unix_ms: i64,
}
