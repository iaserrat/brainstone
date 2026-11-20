use std::pin::Pin;

use agent::{Tool, ToolError, ToolOutput, ToolRequest};
use runtime::{ScheduleRequest, ScheduleStore, ScheduleTime};
use serde_json::Value;
use surface::{Destinations, PostRequest};

use crate::schedule_args::{ScheduleArgs, invalid};

pub(crate) struct ScheduleTool<S, D> {
    pub(crate) store: S,
    pub(crate) min_interval_minutes: u32,
    pub(crate) destinations: D,
}

impl<S: ScheduleStore, D: Destinations> Tool for ScheduleTool<S, D> {
    fn name(&self) -> &'static str {
        "Schedule"
    }

    fn definition(&self) -> Value {
        crate::schedule_definition::definition(self.name(), self.min_interval_minutes)
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move { self.schedule(request).await.map(ToolOutput::from) })
    }
}

impl<S: ScheduleStore, D: Destinations> ScheduleTool<S, D> {
    async fn schedule(&self, request: ToolRequest) -> Result<String, ToolError> {
        let schedule = self.request(request).await?;
        let scheduled = self
            .store
            .schedule(&schedule)
            .await
            .map_err(|error| crate::tool_support::runtime(&error))?;
        serde_json::to_string(&scheduled).map_err(|error| invalid(error.to_string()))
    }

    async fn request(&self, request: ToolRequest) -> Result<ScheduleRequest, ToolError> {
        let operation = crate::tool_support::operation(&request)?;
        let key = request
            .idempotency_key
            .clone()
            .ok_or_else(crate::tool_support::durability)?;
        let identity = request
            .context
            .as_ref()
            .and_then(|context| context.identity.clone());
        let args: ScheduleArgs =
            serde_json::from_value(request.args).map_err(|error| invalid(error.to_string()))?;
        let when = args.time(self.min_interval_minutes)?;
        let standing = matches!(when, ScheduleTime::Repeat(_));
        let schedule = ScheduleRequest::new(operation, &key, &args.prompt, when)
            .map_err(|error| crate::tool_support::runtime(&error))?;
        let Some(conversation) = &args.conversation else {
            return Ok(schedule);
        };
        let post = self.post(operation, identity.as_ref(), conversation, standing);
        Ok(schedule.with_destination(post.await?))
    }

    /// Where the surface lets this requester's runs post in `conversation`.
    async fn post(
        &self,
        operation: runtime::OperationId,
        identity: Option<&agent::ExecutionIdentity>,
        conversation: &str,
        standing: bool,
    ) -> Result<runtime::Destination, ToolError> {
        let identity = identity.ok_or_else(anonymous)?;
        let post = PostRequest {
            operation,
            identity,
            conversation,
            standing,
        };
        self.destinations.resolve(post).await
    }
}

fn anonymous() -> ToolError {
    ToolError::new(
        "schedule_destination_forbidden",
        "posting to another conversation needs an authenticated requester",
    )
}
