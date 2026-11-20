use std::collections::BTreeMap;

use agent::ToolError;
use rmcp::model::{InputRequest, InputRequests, InputResponses};
use serde_json::Value;

use crate::handler::Handler;

pub(crate) async fn fulfill(
    handler: &Handler,
    requests: InputRequests,
) -> Result<InputResponses, ToolError> {
    let mut responses = BTreeMap::new();
    for (id, request) in requests {
        let value = response(handler, &id, request).await?;
        responses.insert(id, value);
    }
    Ok(responses)
}

async fn response(handler: &Handler, id: &str, request: InputRequest) -> Result<Value, ToolError> {
    let InputRequest::Elicitation(request) = request else {
        return Err(ToolError::new(
            "mcp_unsupported_input",
            "sampling and roots input are not supported",
        ));
    };
    let result = handler
        .elicit(request.params, id)
        .await
        .map_err(|error| ToolError::new("mcp_input", error.to_string()))?;
    serde_json::to_value(result)
        .map_err(|error| ToolError::new("mcp_result_encode", error.to_string()))
}
