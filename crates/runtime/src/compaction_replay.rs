//! Attachment bytes and counts the next model call would replay.

/// A thread that is only the attachment prompt itself is exempt: compacting
/// it away would erase the very input it exists to carry.
pub(super) fn replay_attachments(thread: &agent::Thread) -> (usize, usize) {
    if matches!(thread.messages(), [agent::Message::UserImages(_)]) {
        return (0, 0);
    }
    thread
        .messages()
        .iter()
        .flat_map(attachments)
        .fold((0, 0), |counts, images| {
            let bytes = images
                .iter()
                .map(|image| image.bytes().len())
                .sum::<usize>();
            (
                counts.0.saturating_add(images.len()),
                counts.1.saturating_add(bytes),
            )
        })
}

/// The same message with every image replaced by a note saying how many
/// were there.
pub(super) fn without_binaries(message: &agent::Message) -> agent::Message {
    match message {
        agent::Message::UserImages(content) => note(content.text(), content.images().len()),
        agent::Message::ToolResults(results) => {
            agent::Message::ToolResults(results.iter().map(result_without_binaries).collect())
        }
        other => other.clone(),
    }
}

fn result_without_binaries(result: &agent::ToolResult) -> agent::ToolResult {
    agent::ToolResult {
        content: omitted(&result.content, result.images.len()),
        ..result.progress()
    }
}

fn note(text: &str, count: usize) -> agent::Message {
    agent::Message::User(omitted(text, count))
}

fn omitted(text: &str, count: usize) -> String {
    if count == 0 {
        return text.to_owned();
    }
    format!("{text}\n[{count} attached image files omitted from this summary request]")
}

/// Every attachment group one message carries: a user turn has at most one,
/// a tool batch has one per result.
fn attachments(message: &agent::Message) -> impl Iterator<Item = &[agent::Image]> {
    let user = match message {
        agent::Message::UserImages(content) => Some(content.images()),
        _ => None,
    };
    let results: &[agent::ToolResult] = match message {
        agent::Message::ToolResults(results) => results,
        _ => &[],
    };
    user.into_iter()
        .chain(results.iter().map(|result| &result.images[..]))
}
