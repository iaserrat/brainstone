//! What the thread tools share: the thread is always the one the calling
//! operation was accepted in, found through its durable delivery record.

use agent::{ToolError, ToolRequest};
use runtime::{DeliveryStore, DeliveryTarget, OperationId};

use crate::{ThreadHistory, ThreadSource};

/// The calling operation's thread. A model-supplied locator is never used,
/// so no call can read a thread its operation does not belong to.
pub(crate) async fn history<S, D>(
    request: &ToolRequest,
    source: &S,
    store: &D,
) -> Result<ThreadHistory, ToolError>
where
    S: ThreadSource,
    D: DeliveryStore,
    ToolError: From<S::Error>,
{
    let target = thread_target(request, store).await?;
    Ok(source.messages(&target).await?)
}

/// Where the calling operation was accepted, for a surface tool that must
/// act only in that thread.
///
/// # Errors
/// `durable_context_required` outside a durable operation,
/// `thread_unavailable` when the operation has no surface thread, or the
/// store's code.
pub async fn thread_target<D: DeliveryStore>(
    request: &ToolRequest,
    store: &D,
) -> Result<DeliveryTarget, ToolError> {
    store
        .delivery_target(operation(request)?)
        .await
        .map_err(|error| ToolError::new(error.code(), error.to_string()))?
        .ok_or_else(|| {
            ToolError::new(
                "thread_unavailable",
                "this conversation has no surface thread",
            )
        })
}

fn operation(request: &ToolRequest) -> Result<OperationId, ToolError> {
    request
        .context
        .as_ref()
        .and_then(|context| i64::try_from(context.operation_id).ok())
        .and_then(|id| OperationId::new(id).ok())
        .ok_or_else(|| {
            ToolError::new(
                "durable_context_required",
                "this tool requires a durable operation",
            )
        })
}

pub(crate) fn invalid(code: &'static str, error: &impl std::fmt::Display) -> ToolError {
    ToolError::new(code, error.to_string())
}
