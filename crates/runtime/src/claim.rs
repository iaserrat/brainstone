use agent::Thread;

use crate::{Operation, OperationId, SessionId};

/// The exclusive claim one worker holds on an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    /// The operation claimed.
    pub operation_id: OperationId,
    /// The session that stays single-writer.
    pub session_id: SessionId,
    /// The worker identity holding the claim.
    pub worker: String,
    /// The fencing token every write must carry.
    pub token: i64,
}

/// An operation a worker holds the lease for, with what its run needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claimed {
    /// The authenticated caller, when the surface supplied one.
    pub identity: Option<agent::ExecutionIdentity>,
    /// The operation as stored.
    pub operation: Operation,
    /// The thread rebuilt from durable records.
    pub thread: Thread,
    /// The tool names a subagent was spawned with; an empty grant offers none.
    /// `None` offers every tool of the registry that runs the operation.
    pub tool_grant: Option<Vec<String>>,
    /// The claim this work holds.
    pub lease: Lease,
}
