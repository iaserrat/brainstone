//! Posting a schedule's runs as new messages in a surface's conversations.

use agent::{ExecutionIdentity, ToolError};
use runtime::{Destination, OperationId};

/// A caller asking that each run of its schedule post a new message in
/// `conversation` instead of replying where it was asked.
#[derive(Debug)]
pub struct PostRequest<'a> {
    /// The calling operation that asked for the post.
    pub operation: OperationId,
    /// The authenticated principal of the calling operation.
    pub identity: &'a ExecutionIdentity,
    /// The surface's own id for the conversation, as the model named it.
    pub conversation: &'a str,
    /// Whether the schedule repeats, so each post says how to stop it.
    pub standing: bool,
}

/// What a surface implements so a schedule can post in its conversations. The
/// surface decides where posting is allowed; a refusal carries a stable code.
pub trait Destinations: Send + Sync {
    /// Resolves the model-named conversation into a durable delivery target, or
    /// refuses with a stable code the model reads.
    fn resolve(
        &self,
        request: PostRequest<'_>,
    ) -> impl Future<Output = Result<Destination, ToolError>> + Send;
}
