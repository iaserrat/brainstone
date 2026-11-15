use std::collections::BTreeMap;

use agent::{Reasoning, ToolCall};
use serde_json::Value;

use crate::content_block::Block;
use crate::failure::Failure;
use crate::sse::invalid;

pub(crate) struct Output {
    pub(crate) reasoning: Vec<Reasoning>,
    pub(crate) text: String,
    pub(crate) calls: Vec<ToolCall>,
}

impl Output {
    fn new() -> Self {
        Self {
            reasoning: Vec::new(),
            text: String::new(),
            calls: Vec::new(),
        }
    }

    fn push(&mut self, block: Block, truncated: bool) -> Result<(), Failure> {
        match block {
            Block::Text(text) => self.text.push_str(&text),
            Block::Thinking { text, signature } if !signature.is_empty() => {
                self.reasoning.push(Reasoning::Thinking {
                    text,
                    signature: Some(signature),
                });
            }
            Block::Thinking { .. } => return Err(invalid("thinking block has no signature")),
            Block::Redacted(data) => self.reasoning.push(Reasoning::Redacted { data }),
            Block::Tool {
                id,
                name,
                initial,
                json,
            } => self.calls.push(ToolCall {
                id,
                name,
                args: arguments(initial, &json, truncated)?,
            }),
        }
        Ok(())
    }
}

/// `truncated`: the token limit cut the message off, so a tool block may hold
/// unparseable JSON. The call is kept with its initial input so the agent can
/// refuse it explicitly; `StopReason::MaxTokens` already forbids executing it.
pub(crate) fn output(blocks: BTreeMap<usize, Block>, truncated: bool) -> Result<Output, Failure> {
    let mut output = Output::new();
    for block in blocks.into_values() {
        output.push(block, truncated)?;
    }
    Ok(output)
}

fn arguments(initial: Value, json: &str, truncated: bool) -> Result<Value, Failure> {
    let value = match serde_json::from_str(json) {
        _ if json.is_empty() => initial,
        Ok(value) => value,
        Err(_) if truncated => initial,
        Err(error) => return Err(invalid(error.to_string())),
    };
    value
        .is_object()
        .then_some(value)
        .ok_or_else(|| invalid("tool input is not an object"))
}
