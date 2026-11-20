//! A dropped call invalidates its transport; a timeout also sends cancellation.
use crate::handler::Handler;
use agent::ToolError;
use rmcp::{
    RoleClient,
    model::{
        CallToolRequest, CallToolRequestParams, CallToolResponse, CancelledNotificationParam,
        ClientRequest, RequestId, ServerResult,
    },
    service::{PeerRequestOptions, RunningService},
};
use std::time::Duration;

pub(crate) struct Call<'a> {
    service: &'a RunningService<RoleClient, Handler>,
    request_id: Option<RequestId>,
    pub(crate) task_id: Option<String>,
    complete: bool,
    invalidated: &'a std::sync::atomic::AtomicBool,
}

impl<'a> Call<'a> {
    pub(crate) fn new(
        service: &'a RunningService<RoleClient, Handler>,
        invalidated: &'a std::sync::atomic::AtomicBool,
    ) -> Self {
        Self {
            service,
            request_id: None,
            task_id: None,
            complete: false,
            invalidated,
        }
    }

    pub(crate) async fn execute(
        &mut self,
        params: CallToolRequestParams,
    ) -> Result<CallToolResponse, ToolError> {
        let handle = self
            .service
            .send_cancellable_request(
                ClientRequest::CallToolRequest(CallToolRequest::new(params)),
                PeerRequestOptions::no_options(),
            )
            .await
            .map_err(protocol)?;
        tracing::info!(request_id = %handle.id, "MCP request sent");
        self.request_id = Some(handle.id.clone());
        let result = handle.await_response().await.map_err(protocol)?;
        self.request_id = None;
        match result {
            ServerResult::CallToolResult(result) => Ok(CallToolResponse::Complete(result)),
            ServerResult::InputRequiredResult(result) => {
                Ok(CallToolResponse::InputRequired(result))
            }
            ServerResult::CreateTaskResult(result) => Ok(CallToolResponse::Task(result)),
            _ => Err(ToolError::new("mcp_tool_call", "unexpected response")),
        }
    }

    pub(crate) fn complete(&mut self) {
        self.complete = true;
    }

    pub(crate) async fn cancel(&self) {
        // Cancellation is best effort, not evidence that a remote write was rolled back.
        drop(
            tokio::time::timeout(Duration::from_millis(250), async {
                if let Some(id) = &self.request_id {
                    drop(
                        self.service
                            .notify_cancelled(CancelledNotificationParam::new(
                                Some(id.clone()),
                                Some("tool deadline exceeded".into()),
                            ))
                            .await,
                    );
                }
                if let Some(id) = &self.task_id {
                    drop(
                        self.service
                            .cancel_task(rmcp::model::CancelTaskParams::new(id))
                            .await,
                    );
                }
            })
            .await,
        );
    }
}

impl Drop for Call<'_> {
    fn drop(&mut self) {
        self.service.service().set_call_key(None);
        if !self.complete {
            self.invalidated
                .store(true, std::sync::atomic::Ordering::Release);
            self.service.cancellation_token().cancel();
        }
    }
}

fn protocol(error: impl std::fmt::Display) -> ToolError {
    ToolError::new("mcp_tool_call", error.to_string())
}
