use agent::ToolContext;
use rmcp::model::RequestMetaObject;
use serde_json::{Value, json};

pub(crate) const EXECUTION_CONTEXT: &str = "io.brainstone/executionContext";
pub(crate) const IDEMPOTENCY_KEY: &str = "io.brainstone/idempotencyKey";

pub(crate) fn build(context: Option<&ToolContext>, key: Option<String>) -> RequestMetaObject {
    let mut meta = RequestMetaObject::new();
    let identity = context.and_then(|context| context.identity.as_ref());
    meta.0.0.insert(
        EXECUTION_CONTEXT.into(),
        json!({"version": 1, "identity": identity}),
    );
    if let Some(key) = key {
        meta.0.0.insert(IDEMPOTENCY_KEY.into(), Value::String(key));
    }
    if let Some(context) = telemetry::traceparent() {
        meta.0
            .0
            .insert("traceparent".into(), Value::String(context));
    }
    meta
}
