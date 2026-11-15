//! The attachment data model: what a file can become, and what it became.

use std::collections::HashMap;

use agent::Image;

use crate::{AttachmentError, FileRef};

/// What a prompt's attachments became: model image inputs, inlined text
/// files, and per-file skips the model is told about. A skipped file never
/// rejects the prompt; only systemic failures do.
#[derive(Debug, Default)]
pub struct Materialized {
    /// Model image inputs, in prompt order.
    pub images: Vec<Image>,
    /// Inline text files, in prompt order.
    pub texts: Vec<TextAttachment>,
    /// Skip notes for the prompt's own files only.
    pub skipped: Vec<SkippedAttachment>,
    /// Each file's fate, by file id.
    pub statuses: HashMap<String, FileStatus>,
    /// Each file's metadata after resolution, by id: what the surface
    /// described when the copy it started from was partial.
    pub resolved: HashMap<String, FileRef>,
}

impl Materialized {
    /// The `<attachments>` block of inlined text files and skip notes, or
    /// `None` when there is nothing to say.
    #[must_use]
    pub fn block(&self) -> Option<String> {
        crate::attachment_render::render(self)
    }
}

/// One fetched file's fate, in file order. Text carries only its content;
/// every outcome's file name lives on its planned entry.
pub(crate) enum Outcome {
    Image(Image),
    Text(String),
    Skipped(AttachmentError),
    /// Left out only for the prompt's shared budget; fetchable on its own.
    Deferred(AttachmentError),
}

/// What became of one file, keyed by id in [`Materialized::statuses`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// Part of this prompt: an image or document input, or inlined text.
    Attached,
    /// Supported and within its own cap, but past the prompt's budget.
    Available,
    /// Left out, with the reason the model is told.
    Skipped(AttachmentError),
}

#[derive(Debug)]
/// One text file inlined into the prompt.
pub struct TextAttachment {
    /// The file's label: its name when the surface gave one, else its id.
    pub name: String,
    /// The file's text, as fetched.
    pub content: String,
}

#[derive(Debug)]
/// One skipped file, as the prompt's `<attachments>` block reports it.
pub struct SkippedAttachment {
    /// The file's label.
    pub name: String,
    /// Why the file was skipped.
    pub error: AttachmentError,
}

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Image,
    Text,
}

/// The model-input kind a file's declared MIME type maps to; `None` is a
/// kind no surface supports.
pub(crate) fn classify(file: &FileRef) -> Option<Kind> {
    let mime = file.media_type.as_deref().unwrap_or_default();
    if crate::image_validate::media_type(mime).is_ok() {
        return Some(Kind::Image);
    }
    if mime.starts_with("text/") {
        return Some(Kind::Text);
    }
    None
}
