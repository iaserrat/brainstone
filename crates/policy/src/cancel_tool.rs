use std::pin::Pin;

use agent::{Tool, ToolError, ToolOutput, ToolRequest};
use runtime::{ScheduleId, ScheduleStore};
use serde::Deserialize;
use serde_json::{Value, json};

pub(crate) struct CancelSchedule<S>(pub(crate) S);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    id: i64,
}

impl<S: ScheduleStore> Tool for CancelSchedule<S> {
    fn name(&self) -> &'static str {
        "CancelSchedule"
    }

    fn definition(&self) -> Value {
        json!({
            "name": self.name(),
            "description": "Cancel pending scheduled work that ListSchedules shows here.",
            "input_schema": {
                "type": "object",
                "properties": {"id": {"type": "integer", "minimum": 1}},
                "required": ["id"],
                "additionalProperties": false
            }
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            let operation = crate::tool_support::operation(&request)?;
            let args: Args = serde_json::from_value(request.args)
                .map_err(|error| ToolError::new("invalid_schedule_arguments", error.to_string()))?;
            let schedule = self
                .0
                .cancel_schedule(
                    operation,
                    ScheduleId::new(args.id)
                        .map_err(|error| crate::tool_support::runtime(&error))?,
                )
                .await
                .map_err(|error| crate::tool_support::runtime(&error))?;
            serde_json::to_string(&schedule)
                .map_err(|error| ToolError::new("invalid_schedule_result", error.to_string()))
                .map(ToolOutput::from)
        })
    }
}
