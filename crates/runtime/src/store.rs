use std::time::Duration;

use agent::{Event, Message, Outcome};
use serde_json::Value;

use crate::{
    Accepted, Admitted, Claimed, Compaction, Error, Lease, Media, Operation, OperationId, Prompt,
    Settled,
};

/// Atomic persistence operations required by [`Runtime`](crate::Runtime).
pub trait Store: Clone + Send + Sync {
    /// Stores a prompt durably, or returns the operation a retry already stored.
    fn accept(&self, prompt: &Prompt) -> impl Future<Output = Result<Accepted, Error>> + Send;
    /// Stores a prompt together with the tool definitions it advertised.
    fn accept_with_tools(
        &self,
        prompt: &Prompt,
        _definitions: &[Value],
    ) -> impl Future<Output = Result<Accepted, Error>> + Send {
        self.accept(prompt)
    }
    /// Surface admission treats the supplied system prompt as a new-lane
    /// default. Existing lanes keep their stored prompt.
    fn admit_with_tools(
        &self,
        prompt: &Prompt,
        definitions: &[Value],
    ) -> impl Future<Output = Result<Admitted, Error>> + Send;

    /// The stored operation, if it exists.
    fn operation(
        &self,
        id: OperationId,
    ) -> impl Future<Output = Result<Option<Operation>, Error>> + Send;

    /// Claims the next runnable operation for the worker.
    fn claim(
        &self,
        worker: &str,
        ttl: Duration,
    ) -> impl Future<Output = Result<Option<Claimed>, Error>> + Send;

    /// Claims one named operation, for a driver that already knows its id.
    fn claim_operation(
        &self,
        id: OperationId,
        worker: &str,
        ttl: Duration,
    ) -> impl Future<Output = Result<Option<Claimed>, Error>> + Send;

    /// Appends one event to the leased operation's journal.
    fn record(
        &self,
        lease: &Lease,
        event: &Event,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Keep `media` a tool call returned for the conversation's replies.
    /// Named by content, so storing it again after a retry is a no-op.
    fn save_media(
        &self,
        _lease: &Lease,
        _media: &[Media],
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async { Ok(()) }
    }

    /// Steering messages waiting for this lease.
    fn steering(&self, _lease: &Lease) -> impl Future<Output = Result<Vec<Message>, Error>> + Send {
        async { Ok(Vec::new()) }
    }
    /// Required approvals fail closed when a custom store leaves this unsupported.
    fn authorize_tool(
        &self,
        _lease: &Lease,
        _request: &crate::ApprovalRequest,
    ) -> impl Future<Output = Result<crate::ApprovalState, Error>> + Send {
        async { Ok(crate::ApprovalState::Unsupported) }
    }
    /// Non-consuming, cancellation-safe probe. `None` retains boundary-only steering.
    fn has_steering(
        &self,
        _lease: &Lease,
    ) -> impl Future<Output = Result<Option<bool>, Error>> + Send {
        async { Ok(None) }
    }

    /// Records the outcome and releases the lease.
    fn settle(
        &self,
        lease: &Lease,
        outcome: &Outcome,
    ) -> impl Future<Output = Result<Settled, Error>> + Send {
        crate::steering::settle(self, lease, outcome)
    }

    /// Keeps the lease alive for another TTL.
    fn heartbeat(
        &self,
        lease: &Lease,
        ttl: Duration,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Applies a compaction, which becomes the lane's new prefix.
    fn compact(&self, compaction: &Compaction) -> impl Future<Output = Result<(), Error>> + Send;
}
