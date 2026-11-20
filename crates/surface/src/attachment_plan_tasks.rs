//! Per-file planning: every kind walks the same gate — a supported type, a
//! free count slot, a declared size within its cap and the shared total
//! budget, and a usable URL.

use super::{
    Action, Counts, MAX_IMAGE_BYTES, MAX_IMAGES, MAX_TEXT_BYTES, MAX_TEXTS, MAX_TOTAL_BYTES,
};
use crate::attachment::Kind;
use crate::{AttachmentError, FileRef};

pub(super) fn file_action(file: FileRef, counts: &mut Counts) -> Action {
    let Some(kind) = crate::attachment::classify(&file) else {
        return Action::Skip(AttachmentError::Unsupported);
    };
    if *counts.taken(kind) >= capacity(kind) {
        return Action::Defer(AttachmentError::TooMany);
    }
    match budget(&file, byte_cap(kind)) {
        Ok(size) if counts.declared.saturating_add(size) <= MAX_TOTAL_BYTES => {
            *counts.taken(kind) += 1;
            counts.declared = counts.declared.saturating_add(size);
            fetch(kind, file)
        }
        Ok(_) => Action::Defer(AttachmentError::TooLarge),
        Err(error) => Action::Skip(error),
    }
}

impl Counts {
    fn taken(&mut self, kind: Kind) -> &mut usize {
        match kind {
            Kind::Image => &mut self.images,
            Kind::Text => &mut self.texts,
        }
    }
}

const fn capacity(kind: Kind) -> usize {
    match kind {
        Kind::Image => MAX_IMAGES,
        Kind::Text => MAX_TEXTS,
    }
}

const fn byte_cap(kind: Kind) -> usize {
    match kind {
        Kind::Image => MAX_IMAGE_BYTES,
        Kind::Text => MAX_TEXT_BYTES,
    }
}

fn fetch(kind: Kind, file: FileRef) -> Action {
    match kind {
        Kind::Image => Action::Image(file),
        Kind::Text => Action::Text(file),
    }
}

/// A declared size within the per-kind cap, and a private URL to fetch.
fn budget(file: &FileRef, cap: usize) -> Result<usize, AttachmentError> {
    let Some(size) = file.size else {
        return Err(AttachmentError::Unavailable);
    };
    let size = usize::try_from(size).unwrap_or(usize::MAX);
    if size == 0 {
        return Err(AttachmentError::Invalid);
    }
    if size > cap {
        return Err(AttachmentError::TooLarge);
    }
    if file.url.is_none() {
        return Err(AttachmentError::Unavailable);
    }
    Ok(size)
}
