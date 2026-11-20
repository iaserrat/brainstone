//! The two `OpenAI` protocols carry the same durable contract in different fields.
use agent::OutputSchema;
use serde_json::{Value, json};

pub(crate) fn chat(schema: &OutputSchema) -> Value {
    json!({"type":"json_schema", "json_schema":{
        "name":schema.name(), "schema":schema.schema(), "strict":true
    }})
}

pub(crate) fn responses(schema: &OutputSchema) -> Value {
    json!({"format":{
        "type":"json_schema", "name":schema.name(), "schema":schema.schema(), "strict":true
    }})
}
