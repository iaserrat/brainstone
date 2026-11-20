use crate::{
    CHAT_COMPLETIONS_PROVIDER,
    failure::{CODE_FOREIGN_REASONING, Failure},
};
use agent::{Message, ModelResponse, Reasoning, Thread};
use serde_json::{Value, json};

pub(crate) fn messages(thread: &Thread) -> Result<Vec<Value>, Failure> {
    let mut messages = vec![json!({"role":"system", "content":thread.system})];
    for message in thread.messages() {
        messages.extend(items(message)?);
    }
    Ok(messages)
}

#[allow(clippy::too_many_lines, reason = "flat dispatch: one arm per variant")]
fn items(message: &Message) -> Result<Vec<Value>, Failure> {
    match message {
        Message::User(text) | Message::OutputCorrection(text) => {
            Ok(vec![json!({"role":"user", "content":text})])
        }
        Message::Summary(text) => Ok(vec![
            json!({"role":"user", "content":format!("Summary of earlier conversation:\n{text}")}),
        ]),
        Message::UserImages(content) => Ok(vec![crate::chat_parts::user(
            content.text(),
            content.images(),
        )]),
        Message::Assistant(response) => assistant(response).map(|value| vec![value]),
        Message::ToolResults(results) => Ok(results.iter().map(crate::chat_parts::tool).collect()),
    }
}

fn assistant(response: &ModelResponse) -> Result<Value, Failure> {
    let mut message = json!({"role":"assistant", "content":response.text});
    if !response.tool_calls.is_empty() {
        message["tool_calls"] = response.tool_calls.iter().map(|call| json!({
            "id":call.id, "type":"function", "function":{"name":call.name,"arguments":call.args.to_string()}
        })).collect();
    }
    for reasoning in &response.reasoning {
        replay(&mut message, reasoning)?;
    }
    Ok(message)
}

fn replay(message: &mut Value, reasoning: &Reasoning) -> Result<(), Failure> {
    let Reasoning::Opaque { provider, data } = reasoning else {
        return Err(foreign());
    };
    if provider != CHAT_COMPLETIONS_PROVIDER {
        return Err(foreign());
    }
    let fields = data.as_object().ok_or_else(foreign)?;
    for (key, value) in fields {
        if !crate::chat_response::REASONING_FIELDS.contains(&key.as_str()) {
            return Err(foreign());
        }
        message[key] = value.clone();
    }
    Ok(())
}

fn foreign() -> Failure {
    Failure::new(
        CODE_FOREIGN_REASONING,
        "reasoning is not compatible with Chat Completions replay",
    )
}
