//! What the loop does next, and how a run ends.

use serde::{Deserialize, Serialize};

use crate::model::ToolCall;

/// Legacy response count for pre-profile snapshots and explicit finite callers.
/// New execution profiles are unlimited by default.
pub const MAX_STEPS: usize = 100;

/// The run exhausted its explicitly configured response limit.
pub const CODE_STEP_BUDGET: &str = "step_budget_exhausted";

/// A tool call was answered without running, because the response carrying it
/// was cut off by the output token limit.
pub const CODE_TRUNCATED_CALL: &str = "truncated_tool_call";

/// The model asked for a tool no registry entry answers to.
pub const CODE_UNKNOWN_TOOL: &str = "unknown_tool";

/// A replayed tool call was answered without running, because the run that
/// asked for it ended first (a subagent stopped mid-batch, for one).
pub const CODE_UNANSWERED_CALL: &str = "unanswered_tool_call";

/// The tool a model calls to end its turn without replying.
///
/// Declining has to be something the model *does*, not something it omits: an
/// assistant turn carrying no text is dropped from the replayed transcript, so
/// a model that answered correctly by saying nothing cannot see that it did,
/// and reaches for a visible stand-in ("No response.") on the next occasion.
/// A tool call survives replay, which makes the right answer imitable.
pub const SILENCE_TOOL: &str = "Silence";

/// The single unfinished transition of a [`Thread`](crate::Thread).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Step {
    /// Ask the model for its next response.
    CallModel,
    /// Run these calls, in this order.
    RunTools(Vec<ToolCall>),
    /// The response hit the output limit mid-batch, so every call's arguments
    /// may be silently incomplete: streamed arguments are salvage-parsed, so a
    /// truncated call can still parse and validate. Answer them all with an
    /// error instead, and let the model reissue complete calls.
    FailTruncated(Vec<ToolCall>),
    /// The run is over.
    Done(Outcome),
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Outcome {
    /// The model answered the user.
    Completed {
        /// The final assistant text.
        text: String,
    },
    /// The run failed.
    Failed {
        /// A stable slug. Never parse `message` to recover it.
        code: String,
        /// Human-readable detail; `code` is the branching signal.
        message: String,
    },
}

impl Outcome {
    pub(crate) fn failed(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Failed {
            code: code.into(),
            message: message.into(),
        }
    }
}
