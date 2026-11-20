use crate::{ToolContext, ToolError, ToolRequest};

const CODE_DURABILITY_REQUIRED: &str = "tool_durability_required";

/// Caller identity and retry metadata for a typed tool invocation.
///
/// Arguments arrive separately as their Rust type. Metadata comes from the
/// driver, never from model-authored arguments. A retry key enables application
/// deduplication; it does not make an external effect exactly-once by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolInvocation {
    idempotency_key: Option<String>,
    context: Option<ToolContext>,
}

impl ToolInvocation {
    /// The driver's retry identity, stable across durable retries of this call.
    /// Non-durable drivers may provide none.
    #[must_use]
    pub fn idempotency_key(&self) -> Option<&str> {
        self.idempotency_key.as_deref()
    }

    /// Require the driver to have supplied a durable retry identity.
    ///
    /// # Errors
    /// Returns `tool_durability_required` when no key is available. Call this
    /// before effects that require application deduplication.
    pub fn require_idempotency_key(&self) -> Result<&str, ToolError> {
        self.idempotency_key().ok_or_else(|| {
            ToolError::new(
                CODE_DURABILITY_REQUIRED,
                "This tool requires a durable retry key",
            )
        })
    }

    /// The operation, authenticated caller, and accepted execution profile.
    /// Non-durable drivers may provide none.
    #[must_use]
    pub const fn context(&self) -> Option<&ToolContext> {
        self.context.as_ref()
    }
}

impl ToolInvocation {
    pub(crate) fn split(request: ToolRequest) -> (serde_json::Value, Self) {
        let invocation = Self {
            idempotency_key: request.idempotency_key,
            context: request.context,
        };
        (request.args, invocation)
    }
}
