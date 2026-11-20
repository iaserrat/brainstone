//! Pure assembly of per-file outcomes into the materialized attachment set.

use crate::attachment::{FileStatus, Materialized, Outcome, SkippedAttachment, TextAttachment};
use crate::attachment_plan::{MAX_TOTAL_BYTES, Origin, Planned};
use crate::{AttachmentError, attachment::Outcome::Deferred};

pub(crate) fn collect(planned: Vec<Planned>, outcomes: Vec<Outcome>) -> Materialized {
    let mut result = Materialized::default();
    let mut total = 0usize;
    for (entry, outcome) in planned.into_iter().zip(outcomes) {
        let outcome = budget(&mut total, outcome);
        let status = place(&mut result, &entry.origin, outcome);
        let file = entry.origin.file;
        result.statuses.insert(file.id.clone(), status);
        result.resolved.insert(file.id.clone(), file);
    }
    result
}

/// The shared byte budget holds on the stored bytes of every kind, even
/// when declared sizes lied.
fn budget(total: &mut usize, outcome: Outcome) -> Outcome {
    let size = stored_size(&outcome);
    if total.saturating_add(size) > MAX_TOTAL_BYTES {
        return Deferred(AttachmentError::TooLarge);
    }
    *total = total.saturating_add(size);
    outcome
}

/// A deferred thread file stays fetchable; a prompt file that did not make
/// it is a skip note, exactly as before thread files existed.
fn place(result: &mut Materialized, origin: &Origin, outcome: Outcome) -> FileStatus {
    let name = origin.file.label();
    match outcome {
        Outcome::Image(image) => result.images.push(image),
        Outcome::Text(content) => result.texts.push(TextAttachment { name, content }),
        Deferred(_) if origin.thread => return FileStatus::Available,
        Outcome::Skipped(error) | Deferred(error) => {
            if !origin.thread {
                result.skipped.push(SkippedAttachment { name, error });
            }
            return FileStatus::Skipped(error);
        }
    }
    FileStatus::Attached
}

fn stored_size(outcome: &Outcome) -> usize {
    match outcome {
        Outcome::Image(image) => image.bytes().len(),
        Outcome::Text(content) => content.len(),
        Outcome::Skipped(_) | Deferred(_) => 0,
    }
}
