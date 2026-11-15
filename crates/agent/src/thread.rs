//! The conversation state the loop reads.
//!
//! Everything [`next`](crate::next) needs to decide what happens next lives
//! here, which is what lets that decision be a pure function and makes resume
//! "rebuild this and ask again" rather than a second code path.

use serde::{Deserialize, Serialize};

use crate::model::ModelResponse;
use crate::profile::ExecutionProfile;
use crate::tool::ToolResult;
use crate::user_images::UserImages;

#[path = "thread_compaction.rs"]
mod compaction;
#[path = "thread_message.rs"]
mod message;
#[path = "thread_profile.rs"]
mod profile;
#[path = "thread_replay.rs"]
mod replay;

/// One entry the loop replays in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Message {
    /// Text the user sent.
    User(String),
    /// One durable, internal request to correct terminal output. Does not reset budgets.
    OutputCorrection(String),
    /// Text plus images the user sent.
    UserImages(UserImages),
    /// A deliberate replacement for an older conversation prefix.
    Summary(String),
    /// A settled model response.
    Assistant(ModelResponse),
    /// One batch, in the order the assistant requested its calls. Providers
    /// require every call of a batch to be answered before the next turn.
    ToolResults(Vec<ToolResult>),
}

/// A non-empty conversation. Emptiness is unrepresentable because a request
/// with no messages is one every provider rejects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    /// The system prompt, identical for every turn of the lane.
    pub system: String,
    messages: Vec<Message>,
    #[serde(default = "ExecutionProfile::legacy")]
    profile: ExecutionProfile,
    #[serde(default)]
    prior_steps: usize,
}

impl Thread {
    /// A fresh thread carrying one user prompt.
    #[must_use]
    pub fn new(system: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            system: system.into(),
            messages: vec![Message::User(prompt.into())],
            profile: ExecutionProfile::default(),
            prior_steps: 0,
        }
    }

    /// Rebuild a thread from stored messages. `None` when there are none,
    /// which is the invariant this type exists to hold.
    #[must_use]
    pub fn restore(system: impl Into<String>, messages: Vec<Message>) -> Option<Self> {
        if messages.is_empty() {
            return None;
        }
        Some(Self {
            system: system.into(),
            messages,
            profile: ExecutionProfile::default(),
            prior_steps: 0,
        })
    }

    #[must_use]
    /// Every message, oldest first.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// The newest message.
    ///
    /// # Panics
    ///
    /// Never in practice: a `Thread` is non-empty by construction, and both
    /// constructors enforce it. A panic here means that invariant was broken,
    /// which is a bug in this module rather than a caller error.
    #[must_use]
    pub fn last(&self) -> &Message {
        self.messages
            .last()
            .expect("a Thread is non-empty by construction")
    }

    /// Appends the message that just became durable.
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }
}
