//! Pure rendering of the unseen window: one line per message ending in its
//! `<reaction>` tags, then one `<file>` line per file it carries.

use std::fmt::Write as _;

use crate::{FileRef, FileStatus, Materialized, ThreadMessage, Window};

/// Every user-controlled value stays on its message's line, after a fixed
/// `[id] author:` prefix, so a line that starts with `<file` can only come
/// from here. Each file's fate is what `attachments` recorded for it, and
/// `label` turns an author id into its display form.
#[must_use]
pub fn entries(
    window: &Window,
    attachments: &Materialized,
    label: impl Fn(&str) -> String,
) -> String {
    let mut block = String::new();
    if window.omitted > 0 {
        let _ = writeln!(block, "omitted_messages: {}", window.omitted);
    }
    for message in &window.messages {
        let author = sanitize(&label(&message.author));
        let text = sanitize(&message.text);
        let reactions = reactions(message);
        let _ = writeln!(
            block,
            "[{}] {author}: {text}{reactions}",
            sanitize(&message.id)
        );
        for file in &message.files {
            let status = attachments.statuses.get(&file.id);
            let file = attachments.resolved.get(&file.id).unwrap_or(file);
            block.push_str(&file_line(message, file, status));
        }
    }
    block
}

/// A file with no recorded status was never tried, so it is still available.
fn file_line(message: &ThreadMessage, file: &FileRef, status: Option<&FileStatus>) -> String {
    let fate = match status.copied().unwrap_or(FileStatus::Available) {
        FileStatus::Attached => "status=\"attached\"".to_owned(),
        FileStatus::Available => "status=\"available\"".to_owned(),
        FileStatus::Skipped(error) => format!("status=\"skipped\" code=\"{}\"", error.code()),
    };
    format!(
        "<file message=\"{}\" id=\"{}\" name=\"{}\" type=\"{}\" size=\"{}\" {fate}/>\n",
        attribute(&message.id),
        attribute(&file.id),
        attribute(&file.label()),
        attribute(file.media_type.as_deref().unwrap_or_default()),
        file.size.map_or_else(String::new, |size| size.to_string()),
    )
}

fn reactions(message: &ThreadMessage) -> String {
    let mut tags = String::new();
    for reaction in &message.reactions {
        let name = attribute(&reaction.name);
        let (count, own) = (reaction.count, reaction.own);
        let _ = write!(
            tags,
            " <reaction name=\"{name}\" count=\"{count}\" own=\"{own}\"/>"
        );
    }
    tags
}

/// One line that can neither close nor reopen the `<thread_context>` block,
/// nor fake a `<reaction>` tag after the text it sits in.
#[must_use]
pub fn sanitize(text: &str) -> String {
    text.replace('\r', "")
        .replace('\n', "\\n")
        .replace("</thread_context>", "<\\/thread_context>")
        .replace("<thread_context", "<\\thread_context")
        .replace("<reaction", "<\\reaction")
}

fn attribute(value: &str) -> String {
    sanitize(value).replace('"', "'")
}
