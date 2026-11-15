use crate::{failure::Failure, sse::invalid};
use agent::ToolCall;
use serde_json::Value;
use std::collections::HashSet;

pub(crate) fn parse(value: Option<&Value>, truncated: bool) -> Result<Vec<ToolCall>, Failure> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(Vec::new());
    };
    let calls = value
        .as_array()
        .ok_or_else(|| invalid("tool_calls must be an array"))?;
    let mut ids = HashSet::new();
    let mut parsed = Vec::with_capacity(calls.len());
    for call in calls {
        let call = tool(call, truncated)?;
        if !ids.insert(call.id.clone()) {
            return Err(invalid("duplicate tool call identifier"));
        }
        parsed.push(call);
    }
    Ok(parsed)
}

fn tool(value: &Value, truncated: bool) -> Result<ToolCall, Failure> {
    if value["type"] != "function" {
        return Err(invalid("unsupported chat tool call type"));
    }
    let function = &value["function"];
    let arguments = string(function, "arguments")?;
    let args = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) if truncated => Value::Null,
        Err(error) => return Err(invalid(error.to_string())),
    };
    Ok(ToolCall {
        id: string(value, "id")?.to_owned(),
        name: string(function, "name")?.to_owned(),
        args,
    })
}

pub(crate) fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, Failure> {
    value[key]
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(|| invalid(format!("missing {key}")))
}
