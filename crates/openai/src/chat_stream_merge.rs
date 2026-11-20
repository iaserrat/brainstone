use crate::{failure::Failure, sse::invalid};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(crate) fn message(message: &mut Value, delta: &Value) -> Result<(), Failure> {
    for key in ["content", "reasoning", "reasoning_content", "refusal"] {
        if let Some(value) = delta.get(key).filter(|value| !value.is_null()) {
            append(&mut message[key], value)?;
        }
    }
    Ok(())
}

pub(crate) fn indexed(
    items: &mut BTreeMap<u64, Value>,
    delta: Option<&Value>,
) -> Result<(), Failure> {
    let Some(delta) = delta.filter(|value| !value.is_null()) else {
        return Ok(());
    };
    let values = delta
        .as_array()
        .ok_or_else(|| invalid("stream items must be arrays"))?;
    for value in values {
        let index = value["index"]
            .as_u64()
            .ok_or_else(|| invalid("stream item has no index"))?;
        merge(items.entry(index).or_insert_with(|| json!({})), value)?;
    }
    Ok(())
}

fn merge(target: &mut Value, delta: &Value) -> Result<(), Failure> {
    let object = delta
        .as_object()
        .ok_or_else(|| invalid("stream item must be an object"))?;
    for (key, value) in object {
        match key.as_str() {
            "function" => merge(&mut target[key], value)?,
            "arguments" | "name" | "text" | "data" | "signature" | "summary" => {
                append(&mut target[key], value)?;
            }
            _ => target[key] = value.clone(),
        }
    }
    Ok(())
}

fn append(target: &mut Value, delta: &Value) -> Result<(), Failure> {
    let text = delta
        .as_str()
        .ok_or_else(|| invalid("stream fragment must be text"))?;
    let mut previous = target.as_str().unwrap_or_default().to_owned();
    previous.push_str(text);
    *target = Value::String(previous);
    Ok(())
}
