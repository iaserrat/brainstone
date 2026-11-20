use crate::ThreadMessage;

/// The messages a prompt carries because the agent has not seen them, and
/// how many older unseen ones the cap left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// Unseen messages, oldest first, capped to the newest `limit`.
    pub messages: Vec<ThreadMessage>,
    /// How many older unseen messages the cap left out.
    pub omitted: usize,
}

/// Messages after the agent's own last post, excluding the prompt's own
/// message, capped to the newest `limit`. Earlier messages already reached
/// the agent through its durable history.
#[must_use]
pub fn unseen(messages: Vec<ThreadMessage>, prompt: &str, limit: usize) -> Window {
    let seen = messages
        .iter()
        .rposition(|message| message.own)
        .map_or(0, |index| index + 1);
    let mut messages: Vec<_> = messages
        .into_iter()
        .skip(seen)
        .filter(|message| message.id != prompt)
        .collect();
    let omitted = messages.len().saturating_sub(limit);
    messages.drain(..omitted);
    Window { messages, omitted }
}
