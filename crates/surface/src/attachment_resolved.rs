use crate::{AttachmentError, FileRef};

/// Who a file answers to: the file as resolved (its id for status lines,
/// its label for notes and inline text attachments), and whether it came
/// from the thread or the prompt.
#[derive(Clone)]
pub(crate) struct Origin {
    pub(crate) file: FileRef,
    pub(crate) thread: bool,
}

/// A file after metadata resolution: usable metadata or a decided skip.
pub(crate) struct Resolved {
    pub(crate) origin: Origin,
    pub(crate) state: ResolvedState,
}

pub(crate) enum ResolvedState {
    File(FileRef),
    Skipped(AttachmentError),
}

impl Resolved {
    pub(crate) fn file(file: FileRef) -> Self {
        Self {
            origin: origin(&file),
            state: ResolvedState::File(file),
        }
    }

    pub(crate) fn skipped(file: &FileRef, error: AttachmentError) -> Self {
        Self {
            origin: origin(file),
            state: ResolvedState::Skipped(error),
        }
    }

    pub(crate) const fn in_thread(mut self) -> Self {
        self.origin.thread = true;
        self
    }
}

fn origin(file: &FileRef) -> Origin {
    Origin {
        file: file.clone(),
        thread: false,
    }
}
