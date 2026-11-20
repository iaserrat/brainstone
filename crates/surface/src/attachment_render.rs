//! Pure rendering of materialized attachments into the durable prompt text.

use crate::attachment::{Materialized, SkippedAttachment, TextAttachment};

/// Inlined text files and per-file skip notes, appended to the accepted
/// prompt so recovery replays exactly what the model saw. A skip note
/// carries the stable code so the model reports the exact failure instead
/// of the surface rejecting the whole prompt.
pub(crate) fn render(materialized: &Materialized) -> Option<String> {
    if materialized.texts.is_empty() && materialized.skipped.is_empty() {
        return None;
    }
    let mut block = String::from("<attachments>\n");
    for text in &materialized.texts {
        block.push_str(&inline(text));
    }
    for skip in &materialized.skipped {
        block.push_str(&note(skip));
    }
    block.push_str("</attachments>");
    Some(block)
}

fn inline(text: &TextAttachment) -> String {
    format!(
        "<attachment name=\"{}\">\n{}\n</attachment>\n",
        name(&text.name),
        sanitize(&text.content),
    )
}

fn note(skip: &SkippedAttachment) -> String {
    format!(
        "[attachment skipped] name=\"{}\" code=\"{}\" reason=\"{}\"\n",
        name(&skip.name),
        skip.error.code(),
        skip.error,
    )
}

/// File content cannot open or close attachment tags: the block's structure
/// stays unforgeable by anything a user uploads.
fn sanitize(content: &str) -> String {
    content
        .replace("</attachment", "<\\/attachment")
        .replace("<attachment", "<\\attachment")
}

/// A name stays on one line and cannot escape its quotes or forge a tag.
fn name(value: &str) -> String {
    sanitize(&value.replace(['\r', '\n'], " ").replace('"', "'"))
}
