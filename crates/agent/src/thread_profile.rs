use super::{Message, Thread};
use crate::{Effort, ExecutionProfile};

impl Thread {
    /// A fresh thread with a durable execution profile.
    #[must_use]
    pub fn new_with_profile(
        system: impl Into<String>,
        prompt: impl Into<String>,
        profile: ExecutionProfile,
    ) -> Self {
        Self {
            system: system.into(),
            messages: vec![Message::User(prompt.into())],
            profile,
            prior_steps: 0,
            tool_sequence: Some(0),
            tool_successes: Some(Vec::new()),
        }
    }

    /// Rebuild a profiled thread from stored messages.
    #[must_use]
    pub fn restore_with_profile(
        system: impl Into<String>,
        messages: Vec<Message>,
        profile: ExecutionProfile,
    ) -> Option<Self> {
        if messages.is_empty() {
            return None;
        }
        Some(Self {
            system: system.into(),
            messages,
            profile,
            prior_steps: 0,
            tool_sequence: None,
            tool_successes: None,
        })
    }

    /// Restore a thread after a durable in-operation compaction.
    #[must_use]
    pub fn restore_compacted(
        system: impl Into<String>,
        messages: Vec<Message>,
        profile: ExecutionProfile,
        prior_steps: usize,
    ) -> Option<Self> {
        let mut thread = Self::restore_with_profile(system, messages, profile)?;
        thread.prior_steps = prior_steps;
        Some(thread)
    }

    /// The same conversation asked at a different reasoning depth. A call that
    /// is not a turn of the loop needs less thinking than the run does, and
    /// thinking shares one response budget with the answer itself.
    #[must_use]
    pub fn with_effort(mut self, effort: Effort) -> Self {
        self.profile = self.profile.with_effort(effort);
        self
    }

    /// The same conversation for a model that cannot serve a pinned model id.
    #[must_use]
    pub fn without_model(mut self) -> Self {
        self.profile.model = None;
        self
    }

    #[must_use]
    /// The profile this thread's turns run under.
    pub const fn profile(&self) -> &ExecutionProfile {
        &self.profile
    }
}
