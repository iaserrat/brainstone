//! Pure rendering of thread tool results into their structured documents.

use agent::{ToolError, ToolOutput};
use serde_json::{Value, json};

use crate::{FileRef, Materialized, Reaction, ThreadHistory, ThreadMessage};

/// The newest `limit` messages before `before`, oldest first.
pub(crate) fn page(
    history: &ThreadHistory,
    before: Option<&str>,
    limit: usize,
) -> Result<String, ToolError> {
    let end = match before {
        None => history.messages.len(),
        Some(id) => position(history, id)?,
    };
    let start = end.saturating_sub(limit);
    let messages: Vec<Value> = history.messages[start..end].iter().map(message).collect();
    let document = json!({
        "outcome": "thread_read", "complete": history.complete, "more": start > 0,
        "messages": messages,
    });
    Ok(document.to_string())
}

fn position(history: &ThreadHistory, id: &str) -> Result<usize, ToolError> {
    let found = history.messages.iter().position(|message| message.id == id);
    found.ok_or_else(|| {
        ToolError::new(
            "unknown_thread_message",
            "no message with this id in the thread",
        )
    })
}

fn message(message: &ThreadMessage) -> Value {
    json!({
        "id": message.id, "author": message.author, "own": message.own, "text": message.text,
        "files": message.files.iter().map(reference).collect::<Vec<_>>(),
        "reactions": message.reactions.iter().map(reaction).collect::<Vec<_>>(),
    })
}

fn reaction(reaction: &Reaction) -> Value {
    json!({"name": reaction.name, "count": reaction.count, "own": reaction.own})
}

fn reference(file: &FileRef) -> Value {
    json!({"id": file.id, "name": file.label(), "type": file.media_type, "size": file.size})
}

/// One file read as model input, or its skip code as the call's failure.
pub(crate) fn file(file: &FileRef, read: Materialized) -> Result<ToolOutput, ToolError> {
    let file = read.resolved.get(&file.id).unwrap_or(file);
    if let Some(skip) = read.skipped.first() {
        return Err(ToolError::new(skip.error.code(), skip.error.to_string()));
    }
    let mut document = json!({
        "outcome": "file_read", "id": file.id, "name": file.label(), "type": file.media_type,
    });
    if let Some(text) = read.texts.into_iter().next() {
        document["content"] = Value::String(text.content);
    }
    if !read.images.is_empty() {
        document["images"] = read.images.iter().map(media::attachment).collect();
    }
    Ok(ToolOutput {
        text: document.to_string(),
        images: read.images,
        ..ToolOutput::default()
    })
}
