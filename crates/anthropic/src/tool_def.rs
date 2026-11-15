use serde_json::Value;

use crate::failure::Failure;

pub(crate) fn tools(definitions: &[Value]) -> Result<Vec<Value>, Failure> {
    definitions.iter().map(tool).collect()
}

fn tool(definition: &Value) -> Result<Value, Failure> {
    let object = definition.as_object().ok_or_else(invalid_tool)?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(invalid_tool)?;
    let description = object
        .get("description")
        .and_then(Value::as_str)
        .ok_or_else(invalid_tool)?;
    let schema = object
        .get("input_schema")
        .filter(|value| value.is_object())
        .ok_or_else(invalid_tool)?;
    if name.is_empty() || description.is_empty() {
        return Err(invalid_tool());
    }
    Ok(serde_json::json!({
        "name": name, "description": description, "input_schema": schema
    }))
}

fn invalid_tool() -> Failure {
    Failure::new(
        crate::INVALID_TOOL,
        "tool definitions require name, description, and input_schema",
    )
}
