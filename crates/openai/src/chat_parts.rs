use agent::{Image, ToolResult};
use serde_json::{Value, json};

pub(crate) fn user(text: &str, images: &[Image]) -> Value {
    json!({"role":"user", "content":parts(text, images)})
}

pub(crate) fn tool(result: &ToolResult) -> Value {
    let content = if result.images.is_empty() {
        json!(result.content)
    } else {
        json!(parts(&result.content, &result.images))
    };
    json!({"role":"tool", "tool_call_id":result.id, "content":content})
}

fn parts(text: &str, images: &[Image]) -> Vec<Value> {
    let mut parts: Vec<Value> = images.iter().map(|image| json!({
        "type":"image_url", "image_url":{"url":format!("data:{};base64,{}", image.media_type().as_str(), image.base64())}
    })).collect();
    if !text.is_empty() {
        parts.push(json!({"type":"text", "text":text}));
    }
    parts
}
