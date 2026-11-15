//! Concurrent resolution and download of planned attachments.

use futures_util::future::try_join_all;

use crate::attachment::{Materialized, Outcome};
use crate::attachment_collect::collect;
use crate::attachment_plan::{Action, MAX_IMAGE_BYTES, MAX_TEXT_BYTES, Planned, Resolved};
use crate::{AttachmentError, FileRef, ThreadSource};

/// Turn each attachment into a model input or an explicit per-file skip.
/// Only a systemic failure (transport, authentication, rate limit) returns
/// an error and leaves redelivery to the surface; one bad file never costs
/// the rest of the prompt.
///
/// # Errors
/// Returns the source's systemic failure.
pub async fn materialize<S: ThreadSource>(
    files: &[FileRef],
    source: &S,
) -> Result<Materialized, S::Error> {
    run(files, &[], source).await
}

/// The prompt's files, then the thread's, through one set of caps.
pub(crate) async fn run<S: ThreadSource>(
    prompt: &[FileRef],
    thread: &[FileRef],
    source: &S,
) -> Result<Materialized, S::Error> {
    let prompt = prompt.iter().map(|file| resolve(file, source));
    let thread = thread
        .iter()
        .map(|file| async { resolve(file, source).await.map(Resolved::in_thread) });
    let mut resolved = try_join_all(prompt).await?;
    resolved.extend(try_join_all(thread).await?);
    let planned = crate::attachment_plan::plan(resolved);
    let outcomes = try_join_all(planned.iter().map(|entry| fetch(entry, source))).await?;
    Ok(collect(planned, outcomes))
}

async fn resolve<S: ThreadSource>(file: &FileRef, source: &S) -> Result<Resolved, S::Error> {
    if file.is_complete() {
        return Ok(Resolved::file(file.clone()));
    }
    Ok(match source.describe(file).await? {
        Ok(described) => Resolved::file(described),
        Err(error) => Resolved::skipped(file, error),
    })
}

async fn fetch<S: ThreadSource>(entry: &Planned, source: &S) -> Result<Outcome, S::Error> {
    let (file, cap) = match &entry.action {
        Action::Skip(error) => return Ok(Outcome::Skipped(*error)),
        Action::Defer(error) => return Ok(Outcome::Deferred(*error)),
        Action::Image(file) => (file, MAX_IMAGE_BYTES),
        Action::Text(file) => (file, MAX_TEXT_BYTES),
    };
    let outcome = match source.bytes(file, cap).await? {
        Ok(bytes) => validate(&entry.action, bytes),
        Err(error) => Err(error),
    };
    Ok(outcome.unwrap_or_else(Outcome::Skipped))
}

/// Downloaded bytes must be what their declared type claims. A `text/*`
/// payload that is not UTF-8 fails closed, instead of reaching the model as
/// replacement-character mojibake.
fn validate(action: &Action, bytes: Vec<u8>) -> Result<Outcome, AttachmentError> {
    match action {
        Action::Image(file) => crate::image_validate::decode(file, bytes).map(Outcome::Image),
        Action::Text(_) => String::from_utf8(bytes)
            .map(Outcome::Text)
            .map_err(|_| AttachmentError::Invalid),
        Action::Skip(_) | Action::Defer(_) => Err(AttachmentError::Invalid),
    }
}
