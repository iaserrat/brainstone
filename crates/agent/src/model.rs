//! Provider-neutral model output. Every provider crate maps its own wire
//! format into these types, so nothing downstream of the seam knows or cares
//! which provider ran.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One tool invocation requested by the model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Provider-issued, unique within one response. The matching
    /// [`ToolResult`](crate::ToolResult) carries it back.
    pub id: String,
    /// The tool's name, as the provider reported it.
    pub name: String,
    /// The arguments the model sent, parsed from the provider's encoding.
    pub args: Value,
}

/// The request did not fit the model's context window. Every provider crate
/// reports it under this one code because it is the one rejection a caller can
/// repair: a smaller history, not a retry, is what answers it.
pub const CODE_CONTEXT_WINDOW_EXCEEDED: &str = "context_window_exceeded";

/// Why a model response stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    /// The model finished its answer.
    EndTurn,
    /// The model asked for tools; the loop runs them next.
    ToolUse,
    /// The output token limit cut the response off. Any tool calls it carries
    /// may have truncated arguments and must not be executed.
    MaxTokens,
    /// Provider or transport failure, after the provider's own retries are
    /// spent. `code` is the stable slug consumers branch on; `message` is
    /// human-readable detail and is never the branching signal.
    Error {
        /// The stable slug consumers branch on.
        code: String,
        /// Human-readable detail; never the branching signal.
        message: String,
    },
}

/// One extended-thinking block, stored verbatim.
///
/// Signed thinking must replay unmodified and in order in the assistant turn
/// that follows tool use, or the provider rejects the whole request. Editing,
/// reordering, or dropping a block is a correctness bug, not a cosmetic one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reasoning {
    /// Visible thinking. `signature` is the provider's integrity token.
    Thinking {
        /// The visible reasoning text.
        text: String,
        /// The provider's integrity token, replayed with the text.
        signature: Option<String>,
    },
    /// Provider-encrypted thinking, replayed opaquely.
    Redacted {
        /// The provider's opaque payload.
        data: String,
    },
    /// A provider-specific reasoning item whose wire shape must survive
    /// storage unchanged. Only the named provider may replay it.
    Opaque {
        /// The provider that may replay it.
        provider: String,
        /// The provider's wire shape, stored unchanged.
        data: Value,
    },
}

/// A complete, settled model response. The loop appends one of these whole or
/// not at all, so a half-response is never observable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelResponse {
    /// Replayed ahead of `text` and `tool_calls`.
    pub reasoning: Vec<Reasoning>,
    /// The assistant text, already settled.
    pub text: String,
    /// The calls the model asked for, in the order it asked.
    pub tool_calls: Vec<ToolCall>,
    /// Why the response stopped.
    pub stop: StopReason,
    /// The name of the model that produced this response. `None` means the
    /// producer is unknown, and then no model is shown its reasoning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}
