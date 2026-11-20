//! User-turn input parts: images, then optional text.

use serde_json::{Value, json};

pub(super) fn images(text: &str, images: &[agent::Image]) -> Value {
    content(text, images)
}

fn content(text: &str, images: &[agent::Image]) -> Value {
    json!({"role": "user", "content": parts(text, images)})
}

/// The one part order for user turns and function outputs alike.
pub(super) fn parts(text: &str, images: &[agent::Image]) -> Vec<Value> {
    let mut parts: Vec<Value> = images.iter().map(image).collect();
    if !text.is_empty() {
        parts.push(json!({"type": "input_text", "text": text}));
    }
    parts
}

fn image(image: &agent::Image) -> Value {
    let url = format!(
        "data:{};base64,{}",
        image.media_type().as_str(),
        image.base64()
    );
    json!({"type": "input_image", "image_url": url})
}
