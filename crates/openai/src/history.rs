use agent::{Message, ModelResponse, Reasoning, Thread, ToolResult};
use serde_json::{Value, json};

use crate::OPENAI_RESPONSES_PROVIDER;
use crate::failure::{CODE_FOREIGN_REASONING, Failure};

#[path = "history_user.rs"]
mod user;

pub(crate) fn input(thread: &Thread) -> Result<Vec<Value>, Failure> {
    let mut input = Vec::new();
    for message in thread.messages() {
        input.extend(items(message)?);
    }
    Ok(input)
}

fn items(message: &Message) -> Result<Vec<Value>, Failure> {
    match message {
        Message::User(text) | Message::OutputCorrection(text) => {
            Ok(vec![json!({"role": "user", "content": text})])
        }
        Message::UserImages(content) => Ok(vec![user::images(content.text(), content.images())]),
        Message::Summary(text) => Ok(vec![json!({
            "role": "user", "content": format!("Summary of earlier conversation:\n{text}")
        })]),
        Message::Assistant(response) => assistant(response),
        Message::ToolResults(results) => Ok(results.iter().map(tool_result).collect()),
    }
}

fn assistant(response: &ModelResponse) -> Result<Vec<Value>, Failure> {
    let mut items = response
        .reasoning
        .iter()
        .map(reasoning)
        .collect::<Result<Vec<_>, _>>()?;
    if !response.text.is_empty() {
        items.push(json!({"role": "assistant", "content": response.text}));
    }
    for call in &response.tool_calls {
        let arguments = call.args.to_string();
        items.push(json!({
            "type": "function_call", "call_id": call.id,
            "name": call.name, "arguments": arguments
        }));
    }
    Ok(items)
}

fn reasoning(reasoning: &Reasoning) -> Result<Value, Failure> {
    let Reasoning::Opaque { provider, data } = reasoning else {
        return Err(Failure::new(
            CODE_FOREIGN_REASONING,
            "the Responses provider cannot replay another provider's reasoning",
        ));
    };
    if provider != OPENAI_RESPONSES_PROVIDER || data["type"] != "reasoning" {
        return Err(Failure::new(
            CODE_FOREIGN_REASONING,
            "the opaque reasoning item is not from the Responses provider",
        ));
    }
    Ok(data.clone())
}

fn tool_result(result: &ToolResult) -> Value {
    json!({
        "type": "function_call_output", "call_id": result.id, "output": tool_output(result)
    })
}

/// A text-only output stays a bare string, exactly as before attachments.
fn tool_output(result: &ToolResult) -> Value {
    if result.images.is_empty() {
        return Value::String(result.content.clone());
    }
    Value::Array(user::parts(&result.content, &result.images))
}
