use serde_json::Value;

use crate::failure::Failure;
use crate::sse::invalid;

pub(crate) enum Block {
    Text(String),
    Thinking {
        text: String,
        signature: String,
    },
    Redacted(String),
    Tool {
        id: String,
        name: String,
        initial: Value,
        json: String,
    },
}

impl Block {
    pub(crate) fn start(value: &Value) -> Result<Self, Failure> {
        match value["type"].as_str() {
            Some("text") => Ok(Self::Text(field(value, "text")?.to_owned())),
            Some("thinking") => Ok(Self::Thinking {
                text: field(value, "thinking")?.to_owned(),
                signature: field(value, "signature")?.to_owned(),
            }),
            Some("redacted_thinking") => Ok(Self::Redacted(field(value, "data")?.to_owned())),
            Some("tool_use") => tool(value),
            _ => Err(invalid("unknown Anthropic content block")),
        }
    }

    pub(crate) fn delta(&mut self, value: &Value) -> Result<(), Failure> {
        match (self, value["type"].as_str()) {
            (Self::Text(text), Some("text_delta")) => text.push_str(field(value, "text")?),
            (Self::Thinking { text, .. }, Some("thinking_delta")) => {
                text.push_str(field(value, "thinking")?);
            }
            (Self::Thinking { signature, .. }, Some("signature_delta")) => {
                signature.push_str(field(value, "signature")?);
            }
            (Self::Tool { json, .. }, Some("input_json_delta")) => {
                json.push_str(field(value, "partial_json")?);
            }
            _ => return Err(invalid("content delta does not match its block")),
        }
        Ok(())
    }
}

fn tool(value: &Value) -> Result<Block, Failure> {
    Ok(Block::Tool {
        id: field(value, "id")?.to_owned(),
        name: field(value, "name")?.to_owned(),
        initial: value
            .get("input")
            .cloned()
            .ok_or_else(|| invalid("tool block has no input"))?,
        json: String::new(),
    })
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str, Failure> {
    value[name]
        .as_str()
        .ok_or_else(|| invalid(format!("content block {name} is not text")))
}
