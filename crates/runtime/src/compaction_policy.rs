use crate::Error;

#[path = "compaction_replay.rs"]
mod replay;
#[path = "compaction_request.rs"]
mod request;
#[path = "compaction_size.rs"]
mod size;

const MAX_REPLAY_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;
const MAX_REPLAY_ATTACHMENTS: usize = 96;

/// When automatic compaction runs and what it keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionPolicy {
    pub(crate) token_limit: usize,
    pub(crate) prompt: String,
}

impl CompactionPolicy {
    /// # Errors
    /// Returns `invalid_input` for a zero limit or empty summarization prompt.
    pub fn new(token_limit: usize, prompt: &str) -> Result<Self, Error> {
        if token_limit == 0 {
            return Err(Error::InvalidInput {
                field: "auto_compact_token_limit",
            });
        }
        if prompt.is_empty() {
            return Err(Error::InvalidInput {
                field: "compaction_prompt",
            });
        }
        Ok(Self {
            token_limit,
            prompt: prompt.to_owned(),
        })
    }

    #[cfg(test)]
    pub(crate) fn reached(
        &self,
        thread: &agent::Thread,
        tools: &[serde_json::Value],
    ) -> Result<bool, Error> {
        self.reached_with_window(thread, tools, None)
    }

    pub(crate) fn reached_with_window(
        &self,
        thread: &agent::Thread,
        tools: &[serde_json::Value],
        window: Option<usize>,
    ) -> Result<bool, Error> {
        if matches!(
            thread.messages(),
            [agent::Message::Summary(_) | agent::Message::UserImages(_)]
        ) {
            return Ok(false);
        }
        let messages = thread
            .messages()
            .iter()
            .map(size::message_size)
            .try_fold(size::estimate(&thread.system), |total, size| {
                size.map(|value| total.saturating_add(value))
            })?;
        let tools = serde_json::to_string(tools).map_err(Error::corrupt)?;
        Ok(!within_replay_bound(thread)
            || messages + size::estimate(&tools)
                >= window.map_or(self.token_limit, |window| {
                    self.token_limit.min(window.saturating_mul(9) / 10)
                }))
    }

    /// The requests a summary is asked from, most faithful first; the next
    /// one is for a provider that refused the last as too long. They obey the
    /// replay bound every model call obeys: past it, binaries become notes.
    pub(crate) fn summary_requests<'a>(
        &'a self,
        thread: &'a agent::Thread,
    ) -> impl Iterator<Item = agent::Thread> + 'a {
        request::ladder(thread, within_replay_bound(thread))
            .into_iter()
            .map(|rung| request::build(thread, &rung, &self.prompt))
    }
}

fn within_replay_bound(thread: &agent::Thread) -> bool {
    let (count, bytes) = replay::replay_attachments(thread);
    count <= MAX_REPLAY_ATTACHMENTS && bytes <= MAX_REPLAY_ATTACHMENT_BYTES
}
