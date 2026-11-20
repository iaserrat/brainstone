use std::{error, fmt};

/// Why one attachment did not become model input. Each variant has a stable
/// code the model and the skip notes carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentError {
    /// The prompt attached more files of this kind than the model accepts.
    TooMany,
    /// The file exceeds the size cap for its kind.
    TooLarge,
    /// The declared type is neither a supported image nor text.
    Unsupported,
    /// The bytes contradict the declared type or fail to decode.
    Invalid,
    /// The surface could not supply the file.
    Unavailable,
}

impl AttachmentError {
    #[must_use]
    /// The stable code a surface, a log line, and the model branch on.
    pub const fn code(self) -> &'static str {
        match self {
            Self::TooMany => "too_many_attachments",
            Self::TooLarge => "attachment_too_large",
            Self::Unsupported => "unsupported_attachment",
            Self::Invalid => "invalid_attachment",
            Self::Unavailable => "file_unavailable",
        }
    }
}

impl fmt::Display for AttachmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooMany => "the prompt attached more files of this kind than supported",
            Self::TooLarge => "the attachment exceeds the supported size",
            Self::Unsupported => "only JPEG, PNG, or WebP images and text files are supported",
            Self::Invalid => "an attached file is invalid or contradicts its declared type",
            Self::Unavailable => "an attached file is unavailable",
        })
    }
}

impl error::Error for AttachmentError {}
