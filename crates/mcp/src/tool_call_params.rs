use super::RemoteTool;
use agent::{ToolError, ToolRequest};
use rmcp::model::CallToolRequestParams;
use serde_json::Value;
const IDEMPOTENCY_KEY: &str = "io.brainstone/idempotencyKey";
pub(super) fn params(
    tool: &RemoteTool,
    request: ToolRequest,
) -> Result<CallToolRequestParams, ToolError> {
    tool.session.access.check(
        request
            .context
            .as_ref()
            .and_then(|context| context.identity.as_ref()),
    )?;
    let Value::Object(arguments) = request.args else {
        return Err(ToolError::new(
            "mcp_invalid_arguments",
            "expected an object",
        ));
    };
    let mut params = CallToolRequestParams::new(tool.remote_name.clone());
    params.arguments = Some(arguments);
    params.meta = Some(crate::request_meta::build(
        request.context.as_ref(),
        request.idempotency_key,
    ));
    Ok(params)
}

pub(super) fn call_key(params: &CallToolRequestParams) -> Option<String> {
    params
        .meta
        .as_ref()
        .and_then(|meta| meta.0.0.get(IDEMPOTENCY_KEY))
        .and_then(Value::as_str)
        .map(str::to_owned)
}
