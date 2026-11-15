use super::Execution;
use crate::{ApprovalRequest, ApprovalState, Error, Store};
use agent::{Model, ToolCall, ToolResult};
use std::time::Duration;

impl<S: Store, M: Model> Execution<'_, S, M> {
    pub(super) async fn approval(&self, call: &ToolCall) -> Result<Option<ToolResult>, Error> {
        let Some(policy) = self.policy else {
            return Ok(None);
        };
        if policy.denied(&call.name) {
            return Ok(Some(failed(call, "tool_policy_denied")));
        }
        if !policy.required(&call.name) {
            return Ok(None);
        }
        let definition = self
            .definitions
            .iter()
            .find(|value| value["name"] == call.name)
            .cloned()
            .unwrap_or_default();
        let key = self.idempotency_key(call);
        let request =
            ApprovalRequest::new(self.claimed, (&key, call, &definition), &policy.binding)?;
        self.wait_approval(call, &request).await
    }

    async fn wait_approval(
        &self,
        call: &ToolCall,
        request: &ApprovalRequest,
    ) -> Result<Option<ToolResult>, Error> {
        loop {
            if self.store.has_steering(&self.claimed.lease).await? == Some(true) {
                return Ok(Some(failed(call, crate::CODE_TOOL_SUPERSEDED)));
            }
            match self
                .store
                .authorize_tool(&self.claimed.lease, request)
                .await?
            {
                ApprovalState::Approved => {
                    if self.store.has_steering(&self.claimed.lease).await? == Some(true) {
                        return Ok(Some(failed(call, crate::CODE_TOOL_SUPERSEDED)));
                    }
                    return Ok(None);
                }
                ApprovalState::Denied => return Ok(Some(failed(call, "tool_approval_denied"))),
                ApprovalState::Expired => return Ok(Some(failed(call, "tool_approval_expired"))),
                ApprovalState::Unsupported => {
                    return Ok(Some(failed(call, "tool_approval_unsupported")));
                }
                ApprovalState::Pending => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    }
}
fn failed(call: &ToolCall, code: &'static str) -> ToolResult {
    ToolResult {
        id: call.id.clone(),
        name: call.name.clone(),
        content: serde_json::json!({
            "error": code,
            "message": "Tool was not dispatched in this attempt. Respect the operator authorization policy; do not bypass it by changing call IDs."
        }).to_string(),
        is_error: true,
        images: vec![],
    }
}
