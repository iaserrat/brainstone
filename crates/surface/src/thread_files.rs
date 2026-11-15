use std::collections::HashSet;

use crate::{FileRef, Materialized, ThreadSource, Window};

/// The prompt's own files and the window's files through one set of caps:
/// the prompt's first, then the thread's newest first, so the upload just
/// before a mention wins a slot over an old one. A thread file that does not
/// fit stays `Available`; only the prompt's own files get skip notes.
///
/// # Errors
/// Returns the source's systemic failure.
pub async fn materialize_thread<S: ThreadSource>(
    prompt: &[FileRef],
    window: &Window,
    source: &S,
) -> Result<Materialized, S::Error> {
    let mut seen: HashSet<&str> = prompt.iter().map(|file| file.id.as_str()).collect();
    let thread: Vec<FileRef> = window
        .messages
        .iter()
        .rev()
        .flat_map(|message| &message.files)
        .filter(|file| seen.insert(&file.id))
        .cloned()
        .collect();
    crate::attachment_fetch::run(prompt, &thread, source).await
}
