use agent::ToolContext;
use runtime::{Memories, OperationId};

use crate::{Error, Scope};

/// The storage scope owning the calling operation, resolved through the
/// durable records so a tool call cannot write outside its own identity.
pub(crate) async fn resolve<S: Memories>(
    store: &S,
    scope: Scope,
    context: Option<&ToolContext>,
) -> Result<String, Error> {
    let context = context.ok_or(Error::DurableContextRequired)?;
    if !scope.permits(context.identity.as_ref()) {
        return Err(Error::PrivateConversationRequired);
    }
    let operation = i64::try_from(context.operation_id)
        .ok()
        .and_then(|id| OperationId::new(id).ok())
        .ok_or(Error::DurableContextRequired)?;
    let identity = store
        .memory_scope(operation)
        .await
        .map_err(Error::Runtime)?;
    scope
        .key(
            &identity.surface,
            &identity.conversation,
            identity.actor.as_deref(),
        )
        .ok_or(Error::ActorRequired)
}
