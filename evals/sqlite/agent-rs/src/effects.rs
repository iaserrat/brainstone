//! Native tool spec retains the comparison schema exactly.
use crate::protocol::crash;
use agent_rs::{RetryPolicy, Tool, ToolContext, ToolOutput, ToolSpec};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;

struct RecordValue {
    spec: ToolSpec,
    path: String,
    fault: Option<String>,
}

pub fn tool(path: String, fault: Option<String>) -> Arc<dyn Tool> {
    let spec = ToolSpec::new("RecordValue", "1")
        .with_description("Record a value in the application ledger and return it.")
        .with_input_schema(
            json!({"type":"object", "properties":{"value":{"type":"string"}},
            "required":["value"], "additionalProperties":false}),
        )
        .with_retry(RetryPolicy::Idempotent {
            contract: "eval-effects-v1".to_owned(),
        });
    Arc::new(RecordValue { spec, path, fault })
}

#[async_trait]
impl Tool for RecordValue {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn validate(&self, arguments: &Value) -> agent_rs::Result<()> {
        if arguments
            .as_object()
            .is_some_and(|object| object.len() == 1)
            && arguments.get("value").is_some_and(Value::is_string)
        {
            Ok(())
        } else {
            Err(agent_rs::Error::model_output("invalid_arguments"))
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        context: ToolContext,
    ) -> agent_rs::Result<ToolOutput<Value>> {
        let output = crate::ledger::record(&self.path, &context.idempotency_key, &arguments)
            .map_err(agent_rs::Error::tool)?;
        crash(self.fault.as_deref(), "after_effect");
        Ok(ToolOutput::Complete(output))
    }
}
