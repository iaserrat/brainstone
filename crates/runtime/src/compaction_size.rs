//! Size estimation feeding the compaction decision.

use crate::Error;

pub(super) fn message_size(message: &agent::Message) -> Result<usize, Error> {
    match message {
        agent::Message::User(text) | agent::Message::Summary(text) => Ok(estimate(text)),
        agent::Message::UserImages(content) => Ok(content
            .images()
            .iter()
            .map(image_size)
            .fold(estimate(content.text()), usize::saturating_add)),
        agent::Message::ToolResults(results) => results.iter().try_fold(0usize, |total, result| {
            Ok(total.saturating_add(result_size(result)?))
        }),
        _ => serde_json::to_string(message)
            .map(|value| estimate(&value))
            .map_err(Error::corrupt),
    }
}

/// A text-only result is sized as its JSON; one with images as its text
/// plus each image sized the way user attachments are, since base64 is
/// transport, not what the model reads.
fn result_size(result: &agent::ToolResult) -> Result<usize, Error> {
    if result.images.is_empty() {
        let json = serde_json::to_string(result).map_err(Error::corrupt)?;
        return Ok(estimate(&json));
    }
    let text = [&result.id, &result.name, &result.content].map(|value| estimate(value));
    Ok(result
        .images
        .iter()
        .map(image_size)
        .fold(text.iter().sum(), usize::saturating_add))
}

fn image_size(image: &agent::Image) -> usize {
    let width = usize::try_from(image.width()).unwrap_or(usize::MAX);
    let height = usize::try_from(image.height()).unwrap_or(usize::MAX);
    width.div_ceil(28).saturating_mul(height.div_ceil(28))
}

pub(super) fn estimate(value: &str) -> usize {
    value.chars().count().max(value.len().div_ceil(4))
}
