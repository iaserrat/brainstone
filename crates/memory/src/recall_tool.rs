use std::pin::Pin;

use agent::{Tool, ToolError, ToolOutput, ToolRequest};
use runtime::Memories;
use serde_json::{Value, json};

use crate::{Error, Scope};

/// Searches the calling operation's memories, newest first.
#[derive(Debug)]
pub struct RecallTool<S> {
    store: S,
    scope: Scope,
    limit: usize,
}

impl<S> RecallTool<S> {
    #[must_use]
    /// Reads from `store` under `scope`, returning at most `limit` memories.
    pub const fn new(store: S, scope: Scope, limit: usize) -> Self {
        Self {
            store,
            scope,
            limit,
        }
    }
}

#[derive(serde::Deserialize)]
struct RecallArgs {
    #[serde(default)]
    query: Option<String>,
}

impl<S: Memories + 'static> Tool for RecallTool<S> {
    fn name(&self) -> &'static str {
        "MemoryRecall"
    }

    fn definition(&self) -> Value {
        json!({
            "name": self.name(),
            "description": "Recall saved memories for this conversation's scope, newest first. Optionally narrow to a case-insensitive substring.",
            "input_schema": {"type": "object", "additionalProperties": false,
                "properties": {"query": {"type": "string", "description": "Substring to match within memory content."}}}
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            self.recall(request)
                .await
                .map_err(|error| ToolError::new(error.code(), error.to_string()))
                .map(ToolOutput::from)
        })
    }
}

impl<S: Memories> RecallTool<S> {
    async fn recall(&self, request: ToolRequest) -> Result<String, Error> {
        let args: RecallArgs =
            serde_json::from_value(request.args).map_err(|error| Error::InvalidArguments {
                message: error.to_string(),
            })?;
        let scope =
            crate::scope::resolve(&self.store, self.scope, request.context.as_ref()).await?;
        let memories = self
            .store
            .recall_memories(&scope, args.query.as_deref(), self.limit)
            .await
            .map_err(Error::Runtime)?;
        Ok(json!({"memories": memories}).to_string())
    }
}
