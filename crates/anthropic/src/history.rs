use agent::{Message, ModelResponse, Reasoning, Thread, ToolResult};
use serde_json::{Value, json};

use crate::failure::Failure;
use crate::sse::invalid;

#[path = "history_tool_id.rs"]
mod tool_id;
#[path = "history_user.rs"]
mod user;

use tool_id::tool_id;

pub(crate) fn messages(thread: &Thread) -> Result<Vec<Value>, Failure> {
    thread.messages().iter().map(message).collect()
}

fn message(message: &Message) -> Result<Value, Failure> {
    match message {
        Message::User(text) | Message::OutputCorrection(text) => Ok(user::text(text)),
        Message::UserImages(content) => Ok(user::images(content.text(), content.images())),
        Message::Summary(text) => Ok(user::text(&format!(
            "Summary of earlier conversation:\n{text}"
        ))),
        Message::Assistant(response) => assistant(response),
        Message::ToolResults(results) => Ok(tool_results(results)),
    }
}

fn assistant(response: &ModelResponse) -> Result<Value, Failure> {
    let mut content = response
        .reasoning
        .iter()
        .map(reasoning)
        .collect::<Result<Vec<_>, _>>()?;
    if !response.text.is_empty() {
        content.push(json!({"type": "text", "text": response.text}));
    }
    content.extend(response.tool_calls.iter().map(
        |call| json!({"type": "tool_use", "id": tool_id(&call.id), "name": call.name, "input": call.args}),
    ));
    Ok(json!({"role": "assistant", "content": content}))
}

fn reasoning(reasoning: &Reasoning) -> Result<Value, Failure> {
    match reasoning {
        Reasoning::Thinking {
            text,
            signature: Some(signature),
        } => Ok(json!({"type": "thinking", "thinking": text, "signature": signature})),
        Reasoning::Redacted { data } => Ok(json!({"type": "redacted_thinking", "data": data})),
        Reasoning::Thinking {
            signature: None, ..
        }
        | Reasoning::Opaque { .. } => Err(invalid(
            "Anthropic cannot replay foreign or unsigned reasoning",
        )),
    }
}

fn tool_results(results: &[ToolResult]) -> Value {
    let content = results
        .iter()
        .map(|result| {
            json!({
                "type": "tool_result", "tool_use_id": tool_id(&result.id),
                "content": tool_content(result), "is_error": result.is_error
            })
        })
        .collect::<Vec<_>>();
    json!({"role": "user", "content": content})
}

/// A text-only result stays a bare string, byte-identical to what earlier
/// turns sent, so adding attachments never invalidates a cached prefix.
fn tool_content(result: &ToolResult) -> Value {
    if result.images.is_empty() {
        return Value::String(result.content.clone());
    }
    Value::Array(user::blocks(&result.content, &result.images))
}
