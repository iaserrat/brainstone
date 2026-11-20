use crate::{ChatCompletionsModel, failure::Failure};
use agent::Thread;
use serde_json::{Value, json};

pub(crate) fn request(
    model: &ChatCompletionsModel,
    thread: &Thread,
    tools: &[Value],
) -> Result<Value, Failure> {
    let mut request = json!({
        "model": thread.profile().model().unwrap_or(&model.model),
        "messages": crate::chat_history::messages(thread)?,
    });
    if !tools.is_empty() {
        request["tools"] = crate::tool_def::tools(tools)?
            .into_iter()
            .map(nested)
            .collect();
    }
    if let Some(schema) = thread.profile().output_schema() {
        request["response_format"] = crate::output_format::chat(schema);
    }
    if let Some(limit) = model.max_tokens {
        request["max_tokens"] = json!(limit);
    }
    if let Some(effort) = thread.profile().effort().map(Into::into).or(model.effort) {
        request["reasoning_effort"] = json!(effort);
    }
    Ok(request)
}

fn nested(mut definition: Value) -> Value {
    definition
        .as_object_mut()
        .expect("validated tool")
        .remove("type");
    json!({"type":"function", "function":definition})
}
