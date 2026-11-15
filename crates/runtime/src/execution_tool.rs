use agent::{Event, Message, Model, ToolCall, ToolResult};

use super::Execution;
use crate::{Error, Store};

impl<S: Store, M: Model> Execution<'_, S, M> {
    pub(super) async fn call_tools(&mut self, calls: &[ToolCall]) -> Result<(), Error> {
        let mut results = Vec::with_capacity(calls.len());
        for call in calls {
            if self.steer_tools(calls, &mut results).await? {
                return Ok(());
            }
            results.push(self.call_tool(call).await?);
        }
        self.append(Message::ToolResults(results)).await
    }

    pub(super) async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, Error> {
        if let Some(result) = self.approval(call).await? {
            return Ok(result);
        }
        self.record(&Event::ToolStarted {
            id: call.id.clone(),
            name: call.name.clone(),
            args: call.args.clone(),
        })
        .await?;
        let context = self.tool_context()?;
        let key = self.idempotency_key(call);
        let (result, files) =
            agent::execute_tool_with_media(call, &self.tools, Some(key), Some(context)).await;
        let media = crate::stored_media::of(&result.images, files);
        if !media.is_empty() {
            self.store.save_media(&self.claimed.lease, &media).await?;
        }
        self.record(&Event::ToolFinished {
            result: result.progress(),
        })
        .await?;
        Ok(result)
    }

    fn tool_context(&self) -> Result<agent::ToolContext, Error> {
        let operation_id =
            u64::try_from(self.claimed.operation.id.get()).map_err(|_| Error::CorruptRecord {
                message: "negative operation id".to_owned(),
            })?;
        Ok(agent::ToolContext {
            identity: self.claimed.identity.clone(),
            operation_id,
            profile: self.claimed.thread.profile().clone(),
        })
    }

    pub(super) fn idempotency_key(&self, call: &ToolCall) -> String {
        format!(
            "operation/{}/step/{}/call/{}",
            self.claimed.operation.id.get(),
            self.claimed.thread.tool_step(),
            call.id
        )
    }
}
