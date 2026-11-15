use crate::FileRef;

/// Most messages a surface pages through for one thread. Past it the tail is
/// unknown, so the unseen window degrades instead of rendering a stale head.
pub const MAX_THREAD_MESSAGES: usize = 1_000;

/// One message of a surface thread. Authored text and file metadata are
/// untrusted data, never instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMessage {
    /// Unique within the thread; messages arrive oldest first.
    pub id: String,
    /// The surface's id for the user or bot that posted it.
    pub author: String,
    /// Posted by this agent, which is where its unseen window starts.
    pub own: bool,
    /// The authored text, as untrusted data.
    pub text: String,
    /// Files the message carries, in the order the surface reports them.
    pub files: Vec<FileRef>,
    /// In the order the surface reports them.
    pub reactions: Vec<Reaction>,
}

/// One emoji on a message. `name` is untrusted: users create custom emoji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaction {
    /// The emoji name, as the surface reports it.
    pub name: String,
    /// How many users reacted with it.
    pub count: u32,
    /// This agent is one of the reactors.
    pub own: bool,
}

/// A fetched thread, oldest first. `complete` is false when the thread ran
/// past [`MAX_THREAD_MESSAGES`] and only its head was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadHistory {
    /// Messages, oldest first.
    pub messages: Vec<ThreadMessage>,
    /// False when the thread ran past [`MAX_THREAD_MESSAGES`] and only its head
    /// was read.
    pub complete: bool,
}
