//! The tool seam. Every source of tools resolves to the same `dyn Tool`, so
//! the loop holds exactly one dispatch path.

use std::pin::Pin;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ExecutionProfile, Image, ToolOutput};

#[path = "tool_error.rs"]
mod error;
pub use error::ToolError;

/// What one call produced, as the thread stores it and the provider replays it.
/// Empty attachment lists are omitted, so a text-only result keeps the exact
/// shape recorded before attachments existed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    /// The [`ToolCall::id`](crate::ToolCall::id) this answers.
    pub id: String,
    /// The tool's name, as the model calls it.
    pub name: String,
    /// On failure this is the structured error document, not prose.
    pub content: String,
    /// Whether this result is an error the model must read and retry against.
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Images the model reads with this result.
    pub images: Vec<Image>,
}

impl ToolResult {
    /// The copy a progress event carries. Binaries live only in the appended
    /// message, the one record that rebuilds the thread.
    #[must_use]
    pub fn progress(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            content: self.content.clone(),
            is_error: self.is_error,
            images: Vec::new(),
        }
    }
}

/// One tool invocation plus the retry identity supplied by its driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRequest {
    /// The arguments the model sent.
    pub args: Value,
    /// Stable across durable retries. `None` only for a non-durable driver.
    pub idempotency_key: Option<String>,
    /// Durable caller identity and inherited execution settings.
    pub context: Option<ToolContext>,
}

/// The durable context a tool may rely on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolContext {
    /// The operation this call belongs to.
    pub operation_id: u64,
    /// The authenticated caller, when the surface supplied one.
    pub identity: Option<crate::ExecutionIdentity>,
    /// The profile this turn runs under.
    pub profile: ExecutionProfile,
}

/// One callable capability.
///
/// Object-safe on purpose: a registry is a genuinely heterogeneous set, which
/// is the case `dyn` exists for. `call` therefore returns a boxed future
/// instead of using `async fn`.
pub trait Tool: Send + Sync {
    /// The name the model calls this tool by.
    fn name(&self) -> &str;

    /// The complete model-facing definition: `name`, `description`, and
    /// `input_schema`. It lives with the handler so an advertised tool is
    /// always a dispatchable one.
    fn definition(&self) -> Value;

    /// Run the call. Returning `Err` is the ordinary way to fail; the loop
    /// turns it into an error result the model can read and retry against.
    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>>;
}
