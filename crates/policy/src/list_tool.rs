use std::pin::Pin;

use agent::{Tool, ToolError, ToolOutput, ToolRequest};
use runtime::ScheduleStore;
use serde_json::{Value, json};

pub(crate) struct ListSchedules<S>(pub(crate) S);

impl<S: ScheduleStore> Tool for ListSchedules<S> {
    fn name(&self) -> &'static str {
        "ListSchedules"
    }

    fn definition(&self) -> Value {
        json!({
            "name": self.name(),
            "description": "List pending scheduled work: what this conversation created, what posts into it, and, in a conversation private to the requester, every schedule the requester owns. Each item names its owner; paused ones hold their runs until the surface follows that conversation again.",
            "input_schema": {
                "type": "object",
                "properties": {},
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
            let schedules = self
                .0
                .schedules(operation)
                .await
                .map_err(|error| crate::tool_support::runtime(&error))?;
            serde_json::to_string(&schedules)
                .map_err(|error| ToolError::new("invalid_schedule_result", error.to_string()))
                .map(ToolOutput::from)
        })
    }
}
