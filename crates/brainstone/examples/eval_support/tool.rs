use std::pin::Pin;

use brainstone::{Tool, ToolError, ToolOutput, ToolRequest};
use serde_json::{Value, json};
use sqlx::SqlitePool;

pub(crate) struct RecordValue {
    pub(crate) pool: SqlitePool,
    pub(crate) crash_after_effect: bool,
}

impl Tool for RecordValue {
    fn name(&self) -> &'static str {
        "RecordValue"
    }

    fn definition(&self) -> Value {
        json!({"name":self.name(), "description":"Record a value in the application ledger and return it.",
            "input_schema":{"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false}})
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move { self.record(request).await })
    }
}

impl RecordValue {
    async fn record(&self, request: ToolRequest) -> Result<ToolOutput, ToolError> {
        let value = value(&request.args)?;
        let key = request.idempotency_key.as_deref().ok_or_else(|| {
            ToolError::new(
                "eval_missing_retry_key",
                "Durable tools require a retry key.",
            )
        })?;
        let stored = super::ledger::record(&self.pool, key, value)
            .await
            .map_err(|error| ToolError::new("eval_ledger_failed", error.to_string()))?;
        if stored != value {
            return Err(ToolError::new(
                "eval_effect_conflict",
                "A retry changed its value.",
            ));
        }
        if self.crash_after_effect {
            std::process::exit(super::protocol::CRASH);
        }
        Ok(ToolOutput::from(json!({"value":stored}).to_string()))
    }
}

fn value(args: &Value) -> Result<&str, ToolError> {
    let fields = args
        .as_object()
        .filter(|fields| fields.len() == 1)
        .ok_or_else(|| ToolError::new("invalid_arguments", "Supply only a string value."))?;
    fields
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::new("invalid_arguments", "Supply a string value."))
}
