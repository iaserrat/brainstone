use crate::sse::invalid;
use crate::{
    CHAT_COMPLETIONS_PROVIDER,
    failure::{CODE_CONTENT_FILTERED, CODE_REFUSED, Failure},
};
use agent::{ModelResponse, Reasoning, StopReason};
use serde_json::{Map, Value};

pub(crate) const REASONING_FIELDS: [&str; 3] =
    ["reasoning", "reasoning_content", "reasoning_details"];

pub(crate) fn parse(value: &Value) -> Result<ModelResponse, Failure> {
    if value.get("error").is_some() {
        return Err(crate::chat_stream::rejection(value));
    }
    let choices = value["choices"]
        .as_array()
        .filter(|choices| choices.len() == 1)
        .ok_or_else(|| invalid("expected exactly one chat choice"))?;
    settled(&choices[0]["message"], &choices[0]["finish_reason"])
}

pub(crate) fn settled(message: &Value, finish: &Value) -> Result<ModelResponse, Failure> {
    let stop = stop(finish)?;
    validate_message(message)?;
    let text = text(message.get("content"))?;
    let tool_calls =
        crate::chat_tools::parse(message.get("tool_calls"), stop == StopReason::MaxTokens)?;
    if stop == StopReason::ToolUse && tool_calls.is_empty() {
        return Err(invalid("tool finish without calls"));
    }
    if stop == StopReason::EndTurn && !tool_calls.is_empty() {
        return Err(invalid("calls without tool finish"));
    }
    Ok(ModelResponse {
        model: None,
        text,
        reasoning: reasoning(message),
        tool_calls,
        stop,
    })
}

fn stop(value: &Value) -> Result<StopReason, Failure> {
    match value.as_str() {
        Some("stop") => Ok(StopReason::EndTurn),
        Some("tool_calls") => Ok(StopReason::ToolUse),
        Some("length") => Ok(StopReason::MaxTokens),
        Some("content_filter") => Err(Failure::new(
            CODE_CONTENT_FILTERED,
            "provider filtered the response",
        )),
        _ => Err(invalid("unknown or missing chat finish reason")),
    }
}

fn text(content: Option<&Value>) -> Result<String, Failure> {
    match content {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(text)) => Ok(text.clone()),
        _ => Err(invalid("chat content must be text or null")),
    }
}

fn reasoning(message: &Value) -> Vec<Reasoning> {
    let fields: Map<String, Value> = REASONING_FIELDS
        .into_iter()
        .filter_map(|key| {
            message
                .get(key)
                .filter(|value| !value.is_null())
                .map(|value| (key.to_owned(), value.clone()))
        })
        .collect();
    if fields.is_empty() {
        return Vec::new();
    }
    vec![Reasoning::Opaque {
        provider: CHAT_COMPLETIONS_PROVIDER.to_owned(),
        data: Value::Object(fields),
    }]
}

fn validate_message(message: &Value) -> Result<(), Failure> {
    if ["audio", "images", "video"]
        .iter()
        .any(|key| message.get(*key).is_some_and(|value| !value.is_null()))
    {
        return Err(invalid("unsupported chat output modality"));
    }
    if message.get("refusal").is_some_and(|value| !value.is_null()) {
        return Err(Failure::new(CODE_REFUSED, "model refused the request"));
    }
    Ok(())
}
