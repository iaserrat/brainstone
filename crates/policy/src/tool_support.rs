use agent::{ToolError, ToolRequest};
use runtime::OperationId;

pub(crate) fn operation(request: &ToolRequest) -> Result<OperationId, ToolError> {
    let context = request.context.as_ref().ok_or_else(durability)?;
    i64::try_from(context.operation_id)
        .ok()
        .and_then(|value| OperationId::new(value).ok())
        .ok_or_else(|| {
            ToolError::new(
                "invalid_schedule_arguments",
                "operation identity is invalid",
            )
        })
}

pub(crate) fn durability() -> ToolError {
    ToolError::new(
        "schedule_requires_durability",
        "scheduled work requires a durable operation identity",
    )
}

pub(crate) fn runtime(error: &runtime::Error) -> ToolError {
    ToolError::new(error.code(), error.to_string())
}
