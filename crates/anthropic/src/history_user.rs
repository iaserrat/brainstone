//! User-turn content blocks: images, then optional text.

use serde_json::{Value, json};

pub(super) fn text(text: &str) -> Value {
    json!({"role": "user", "content": [{"type": "text", "text": text}]})
}

pub(super) fn images(text: &str, images: &[agent::Image]) -> Value {
    content(text, images)
}

fn content(text: &str, images: &[agent::Image]) -> Value {
    json!({"role": "user", "content": blocks(text, images)})
}

/// The one block order for user turns and tool results alike.
pub(super) fn blocks(text: &str, images: &[agent::Image]) -> Vec<Value> {
    let mut blocks: Vec<Value> = images.iter().map(image).collect();
    if !text.is_empty() {
        blocks.push(json!({"type": "text", "text": text}));
    }
    blocks
}

fn image(image: &agent::Image) -> Value {
    json!({
        "type": "image",
        "source": {"type": "base64", "media_type": image.media_type().as_str(),
                   "data": image.base64()}
    })
}
