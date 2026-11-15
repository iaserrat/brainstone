use crate::{Error, OperationId};

/// One durable memory: content an agent keeps across the sessions of a scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Memory {
    pub(crate) scope: String,
    pub(crate) key: String,
    pub(crate) content: String,
}

impl Memory {
    /// # Errors
    ///
    /// Returns `invalid_input` when the scope, key, or content is empty.
    pub fn new(scope: &str, key: &str, content: &str) -> Result<Self, Error> {
        crate::prompt::require(scope, "memory_scope")?;
        crate::prompt::require(key, "memory_key")?;
        crate::prompt::require(content, "memory_content")?;
        Ok(Self {
            scope: scope.to_owned(),
            key: key.to_owned(),
            content: content.to_owned(),
        })
    }
}

/// The identity owning an operation, for memory-scope derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryScope {
    /// The surface the scope belongs to.
    pub surface: String,
    /// The conversation the scope belongs to.
    pub conversation: String,
    /// The surface user who authored the accepted prompt, when known.
    pub actor: Option<String>,
}

/// Cross-session memory, the one deliberately shared state between sessions.
/// A scope names the owner (for example a surface and conversation), so what
/// one session saves, every later session of that scope can recall.
pub trait Memories: Clone + Send + Sync {
    /// Persist one memory. Replaying the same key with identical input is a
    /// no-op; reusing it with different input is `idempotency_conflict`.
    fn save_memory(&self, memory: &Memory) -> impl Future<Output = Result<(), Error>> + Send;

    /// Latest memories for `scope`, newest first, capped at `limit` and
    /// optionally narrowed to a case-insensitive substring of the content.
    fn recall_memories(
        &self,
        scope: &str,
        filter: Option<&str>,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<String>, Error>> + Send;

    /// The surface, conversation, and actor owning `operation`.
    fn memory_scope(
        &self,
        operation: OperationId,
    ) -> impl Future<Output = Result<MemoryScope, Error>> + Send;
}
