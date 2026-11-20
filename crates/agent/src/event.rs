//! Everything a run makes observable.
//!
//! This enum is the entire output contract. It serialises to JSON, so a
//! surface that speaks JSON needs no trait of its own, and a durable store is
//! just a [`Sink`] that writes what it is handed.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::step::Outcome;
use crate::thread::Message;
use crate::tool::ToolResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
/// Everything one run makes observable. Serialized, so a JSON surface needs
/// no trait of its own and a durable store is a `Sink`.
pub enum Event {
    /// The run started; nothing is durable yet.
    RunStarted,
    /// Provisional assistant text for live surfaces. It never rebuilds model
    /// context; only the settled `Appended` assistant message does that.
    ModelDelta {
        /// Text produced so far in this response.
        delta: String,
    },
    /// Provisional, provider-visible reasoning text for live surfaces. Encrypted
    /// payloads and signatures remain only in the settled assistant message.
    /// This progress event never rebuilds model context.
    ReasoningDelta {
        /// Reasoning text produced so far in this response.
        delta: String,
    },
    /// A message joined the thread.
    ///
    /// These alone rebuild the conversation: replay every `Appended` in order
    /// and you hold what the loop holds. That is the whole durability
    /// contract, and why this variant carries the message rather than a
    /// summary of it. The events below report progress and rebuild nothing.
    Appended {
        /// The message that joined the thread.
        message: Message,
    },
    /// A deliberate replacement of the model-visible prefix.
    Compacted {
        /// The summary that replaced the model-visible prefix.
        summary: String,
        /// Response count the summary replaced.
        steps: usize,
    },
    /// A tool call is about to run. Never emitted for a call the loop refused
    /// to run, because nothing started.
    ToolStarted {
        /// The provider's id for the call.
        id: String,
        /// The tool's name.
        name: String,
        /// The arguments the model sent.
        args: Value,
    },
    /// Carries the result without its images: only the appended
    /// `ToolResults` message stores those.
    ToolFinished {
        /// The result, without the images only the appended message stores.
        result: ToolResult,
    },
    /// The run ended.
    RunFinished {
        /// How it ended.
        outcome: Outcome,
    },
}

/// Where events go.
pub trait Sink {
    /// What recording an event can fail with. It is fallible because the
    /// intended implementation is a durable store, and a run must not continue
    /// past a write that did not land.
    type Error: Send;

    /// Emits one event, and fails when it was not durably recorded.
    fn emit(&mut self, event: Event) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
