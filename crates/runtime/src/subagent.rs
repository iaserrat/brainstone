use serde::{Deserialize, Serialize};

use crate::{Error, Operation, OperationId, SubagentRequest};

/// The store's id for a child run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubagentId(pub(crate) i64);

impl SubagentId {
    /// # Errors
    ///
    /// Returns `invalid_input` unless the identifier is positive.
    pub fn new(value: i64) -> Result<Self, Error> {
        (value > 0)
            .then_some(Self(value))
            .ok_or(Error::InvalidInput { field: "agent_id" })
    }

    #[must_use]
    /// The raw id.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// One child run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subagent {
    /// Its id.
    pub id: SubagentId,
    /// The child's own operation.
    pub operation: Operation,
    /// The definition name the child runs.
    pub name: String,
    /// Whether it runs detached.
    pub background: bool,
}

/// The result of spawning a child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnedSubagent {
    /// The child run.
    pub agent: Subagent,
    /// Whether a retry had already spawned it.
    pub duplicate: bool,
}

/// Durable child runs: spawn, read, message, and stop.
pub trait SubagentStore: Clone + Send + Sync {
    /// Starts a child run, or returns the one the same key already started.
    fn spawn_subagent(
        &self,
        request: &SubagentRequest,
    ) -> impl Future<Output = Result<SpawnedSubagent, Error>> + Send;

    /// The child run, if it exists.
    fn subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
    ) -> impl Future<Output = Result<Option<Subagent>, Error>> + Send;

    /// Adds a task message to a child run.
    fn message_subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
        key: &str,
        message: &str,
    ) -> impl Future<Output = Result<Subagent, Error>> + Send;

    /// Asks a child run to stop.
    fn stop_subagent(
        &self,
        caller: OperationId,
        id: SubagentId,
    ) -> impl Future<Output = Result<Subagent, Error>> + Send;
}
