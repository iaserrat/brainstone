//! Pure planning of which attachments to download, decided before any
//! network work.

use crate::{AttachmentError, FileRef};

#[path = "attachment_resolved.rs"]
mod resolved;
#[path = "attachment_plan_tasks.rs"]
mod tasks;

pub(crate) use resolved::{Origin, Resolved, ResolvedState};

pub(crate) const MAX_IMAGES: usize = 4;
pub(crate) const MAX_TEXTS: usize = 4;
pub(crate) const MAX_IMAGE_BYTES: usize = media::MAX_IMAGE_BYTES;
pub(crate) const MAX_TEXT_BYTES: usize = 256 * 1024;
/// Attachments of every kind share this byte budget, enforced on declared
/// sizes at planning and on stored bytes at collection, so one prompt's
/// base64 payload stays inside the provider request cap.
pub(crate) const MAX_TOTAL_BYTES: usize = 10 * 1024 * 1024;

/// One file's plan: the origin every note stays attributable to, and the
/// download to perform or the already-decided skip.
pub(crate) struct Planned {
    pub(crate) origin: Origin,
    pub(crate) action: Action,
}

pub(crate) enum Action {
    Image(FileRef),
    Text(FileRef),
    Skip(AttachmentError),
    /// Within its own cap but past the prompt's shared count or byte budget.
    Defer(AttachmentError),
}

/// Decide per file, in order, what to download: kind support, per-kind count
/// caps, declared byte budgets, and a usable URL. Deciding before any
/// download keeps the byte caps bounds on network work, not just on memory.
pub(crate) fn plan(resolved: Vec<Resolved>) -> Vec<Planned> {
    let mut counts = Counts::default();
    resolved
        .into_iter()
        .map(|entry| Planned {
            action: match entry.state {
                ResolvedState::File(file) => tasks::file_action(file, &mut counts),
                ResolvedState::Skipped(error) => Action::Skip(error),
            },
            origin: entry.origin,
        })
        .collect()
}

#[derive(Default)]
struct Counts {
    images: usize,
    texts: usize,
    declared: usize,
}
