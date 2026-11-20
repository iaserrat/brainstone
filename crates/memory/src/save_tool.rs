use std::pin::Pin;

use agent::{Tool, ToolError, ToolOutput, ToolRequest};
use runtime::{Memories, Memory};
use serde_json::{Value, json};

use crate::{Error, Scope};

/// Persists one memory owned by the calling operation's scope. The durable
/// idempotency key makes a crash-retried call save exactly one memory.
#[derive(Debug)]
pub struct SaveTool<S> {
    store: S,
    scope: Scope,
}

impl<S> SaveTool<S> {
    #[must_use]
    /// Saves into `store` under `scope`.
    pub const fn new(store: S, scope: Scope) -> Self {
        Self { store, scope }
    }
}

#[derive(serde::Deserialize)]
struct SaveArgs {
    content: String,
}

impl<S: Memories + 'static> Tool for SaveTool<S> {
    fn name(&self) -> &'static str {
        "MemorySave"
    }

    fn definition(&self) -> Value {
        json!({
            "name": self.name(),
            "description": "Persist one durable memory for future sessions of this conversation's scope. Save stable facts, preferences, and corrections, not transient task state.",
            "input_schema": {"type": "object", "additionalProperties": false, "required": ["content"],
                "properties": {"content": {"type": "string", "description": "One self-contained memory."}}}
        })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            self.save(request)
                .await
                .map_err(|error| ToolError::new(error.code(), error.to_string()))
                .map(ToolOutput::from)
        })
    }
}

impl<S: Memories> SaveTool<S> {
    async fn save(&self, request: ToolRequest) -> Result<String, Error> {
        let args: SaveArgs =
            serde_json::from_value(request.args).map_err(|error| Error::InvalidArguments {
                message: error.to_string(),
            })?;
        let key = request
            .idempotency_key
            .as_deref()
            .ok_or(Error::DurableContextRequired)?;
        let scope =
            crate::scope::resolve(&self.store, self.scope, request.context.as_ref()).await?;
        let memory = Memory::new(&scope, key, &args.content).map_err(Error::Runtime)?;
        self.store
            .save_memory(&memory)
            .await
            .map_err(Error::Runtime)?;
        Ok(json!({"saved": true}).to_string())
    }
}
