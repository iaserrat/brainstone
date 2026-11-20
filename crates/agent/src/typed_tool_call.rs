use super::TypedTool;
use crate::{MAX_TOOL_RESULT_BYTES, Tool, ToolError, ToolInvocation, ToolOutput, ToolRequest};
use schemars::JsonSchema;
use serde::{Serialize, de::DeserializeOwned};
use std::pin::Pin;

const CODE_OUTPUT_INVALID: &str = "tool_output_invalid";
const CODE_INVALID_ARGUMENTS: &str = "invalid_tool_arguments";

impl<A, O, E, F, Fut> Tool for TypedTool<A, F>
where
    A: DeserializeOwned + JsonSchema + Send + 'static,
    O: Serialize,
    E: Into<ToolError>,
    F: Fn(A, ToolInvocation) -> Fut + Send + Sync,
    Fut: Future<Output = Result<O, E>> + Send,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn definition(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "input_schema": self.schema.schema(),
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            let (args, invocation) = ToolInvocation::split(request);
            let args = self.arguments(args)?;
            let value = (self.handler)(args, invocation).await.map_err(Into::into)?;
            let output = serde_json::to_string(&Success {
                outcome: "success",
                value,
            })
            .map_err(|error| ToolError::new(CODE_OUTPUT_INVALID, error.to_string()))?;
            if output.len() > MAX_TOOL_RESULT_BYTES {
                return Err(ToolError::new(
                    CODE_OUTPUT_INVALID,
                    "Serialized tool output exceeds the result byte limit",
                ));
            }
            Ok(output.into())
        })
    }
}

impl<A: DeserializeOwned, F> TypedTool<A, F> {
    fn arguments(&self, args: serde_json::Value) -> Result<A, ToolError> {
        self.schema
            .validate(&args.to_string())
            .map_err(|error| ToolError::new(CODE_INVALID_ARGUMENTS, error.to_string()))?;
        serde_json::from_value(args)
            .map_err(|error| ToolError::new(CODE_INVALID_ARGUMENTS, error.to_string()))
    }
}

#[derive(Serialize)]
struct Success<T> {
    outcome: &'static str,
    value: T,
}
