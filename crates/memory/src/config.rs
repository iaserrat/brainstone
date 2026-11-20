/// Most memories one `MemoryRecall` call returns unless configured otherwise.
const DEFAULT_RECALL_LIMIT: usize = 20;

/// Operator control over when memory triggers and which identity owns it.
/// Everything defaults to off: an unconfigured agent has no memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Which identity owns saved memories.
    pub scope: Scope,
    /// Expose the `MemorySave` tool so the model can persist memories.
    pub save: bool,
    /// Expose the `MemoryRecall` tool so the model can search memories.
    pub recall_tool: bool,
    /// Most memories one `MemoryRecall` call returns.
    pub recall_limit: usize,
    /// Prepend this many recent memories to every accepted prompt.
    /// `None` leaves prompts untouched.
    pub inject: Option<usize>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scope: Scope::Conversation,
            save: false,
            recall_tool: false,
            recall_limit: DEFAULT_RECALL_LIMIT,
            inject: None,
        }
    }
}

/// The identity that owns a memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Each conversation remembers independently.
    Conversation,
    /// Every conversation on a surface shares one memory.
    Surface,
    /// Each surface user remembers independently, across conversations.
    User,
}

impl Scope {
    #[must_use]
    /// Whether an operation carrying `identity` may use this scope.
    pub fn permits(self, identity: Option<&agent::ExecutionIdentity>) -> bool {
        self != Self::User
            || identity.is_none_or(|identity| identity.is_valid() && identity.private)
    }

    /// The storage scope for an operation of this identity, or `None` when
    /// the scope needs an actor the surface did not supply. The user key uses
    /// `:` so no conversation id can collide with it.
    #[must_use]
    pub fn key(self, surface: &str, conversation: &str, actor: Option<&str>) -> Option<String> {
        match self {
            Self::Conversation => Some(format!("{surface}/{conversation}")),
            Self::Surface => Some(surface.to_owned()),
            Self::User => actor.map(|actor| format!("{surface}:user:{actor}")),
        }
    }
}
