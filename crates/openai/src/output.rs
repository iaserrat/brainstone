use agent::{Reasoning, ToolCall};
use serde_json::Value;

use crate::OPENAI_RESPONSES_PROVIDER;
use crate::failure::{CODE_INVALID_RESPONSE, CODE_REFUSED, Failure};

pub(crate) struct ParsedOutput {
    pub(crate) reasoning: Vec<Reasoning>,
    pub(crate) text: String,
    pub(crate) tool_calls: Vec<ToolCall>,
}

pub(crate) fn parse(items: &[Value], truncated: bool) -> Result<ParsedOutput, Failure> {
    let mut output = ParsedOutput {
        reasoning: Vec::new(),
        text: String::new(),
        tool_calls: Vec::new(),
    };
    for item in items {
        match string(item, "type")? {
            "reasoning" => output.reasoning.push(Reasoning::Opaque {
                provider: OPENAI_RESPONSES_PROVIDER.to_owned(),
                data: item.clone(),
            }),
            "message" => message(item, &mut output.text)?,
            "function_call" => output.tool_calls.push(function_call(item, truncated)?),
            kind => return Err(invalid(format!("unsupported output item: {kind}"))),
        }
    }
    Ok(output)
}

fn message(item: &Value, text: &mut String) -> Result<(), Failure> {
    let content = item["content"]
        .as_array()
        .ok_or_else(|| invalid("message content must be an array"))?;
    for part in content {
        match string(part, "type")? {
            "output_text" => text.push_str(string(part, "text")?),
            "refusal" => return Err(Failure::new(CODE_REFUSED, string(part, "refusal")?)),
            kind => return Err(invalid(format!("unsupported message content: {kind}"))),
        }
    }
    Ok(())
}

fn function_call(item: &Value, truncated: bool) -> Result<ToolCall, Failure> {
    let arguments = string(item, "arguments")?;
    let args = match serde_json::from_str(arguments) {
        Ok(args) => args,
        Err(_) if truncated => Value::Null,
        Err(error) => return Err(invalid(format!("invalid function arguments: {error}"))),
    };
    Ok(ToolCall {
        id: string(item, "call_id")?.to_owned(),
        name: string(item, "name")?.to_owned(),
        args,
    })
}

fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str, Failure> {
    value[field]
        .as_str()
        .ok_or_else(|| invalid(format!("{field} must be a string")))
}

fn invalid(message: impl Into<String>) -> Failure {
    Failure::new(CODE_INVALID_RESPONSE, message)
}
