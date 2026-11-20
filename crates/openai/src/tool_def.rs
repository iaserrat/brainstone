use serde_json::{Map, Value};

use crate::failure::{CODE_INVALID_TOOL, Failure};

pub(crate) fn tools(definitions: &[Value]) -> Result<Vec<Value>, Failure> {
    definitions.iter().map(tool).collect()
}

fn tool(definition: &Value) -> Result<Value, Failure> {
    let object = definition.as_object().ok_or_else(invalid)?;
    let name = text(object, "name")?;
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
    {
        return Err(invalid());
    }
    let description = text(object, "description")?;
    let parameters = object
        .get("input_schema")
        .filter(|schema| schema.is_object())
        .ok_or_else(invalid)?;
    let mut converted = Map::new();
    converted.insert("type".to_owned(), Value::String("function".to_owned()));
    converted.insert("name".to_owned(), Value::String(name.to_owned()));
    converted.insert(
        "description".to_owned(),
        Value::String(description.to_owned()),
    );
    converted.insert("parameters".to_owned(), parameters.clone());
    Ok(Value::Object(converted))
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Failure> {
    object.get(key).and_then(Value::as_str).ok_or_else(invalid)
}

fn invalid() -> Failure {
    Failure::new(
        CODE_INVALID_TOOL,
        "tool definitions require name, description, and input_schema",
    )
}
