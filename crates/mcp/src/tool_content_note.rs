//! The note that stands in for a binary block the model does not receive.
//! The model reads a result as text, so base64 would cost context for
//! nothing: a 2 MB PDF is 2.7 MB of it.

use rmcp::model::ContentBlock;

use crate::tool_content_image::decoded_len;

/// What a blob resource that declares no media type is.
pub(crate) const UNKNOWN_MEDIA_TYPE: &str = "application/octet-stream";

/// Why a binary block was left out. Each code opens its note, so the model
/// branches on it rather than on the prose after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Omission {
    /// Audio, or any binary in an error result.
    Binary,
    /// An image that is not PNG, JPEG, or WebP.
    ImageUnsupported,
    /// An image that is not valid base64 or does not decode as its type.
    ImageInvalid,
    /// An image past the per-image byte cap or the decode bound.
    ImageTooLarge,
    /// A video blob resource, which the reply cannot show.
    VideoUnsupported,
    /// A blob that is not valid base64, or whose URI or type names no file.
    FileInvalid,
    /// A blob past the per-file byte cap.
    FileTooLarge,
}

impl Omission {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Binary => "mcp_binary_omitted",
            Self::ImageUnsupported => "mcp_image_unsupported",
            Self::ImageInvalid => "mcp_image_invalid",
            Self::ImageTooLarge => "mcp_image_too_large",
            Self::VideoUnsupported => "mcp_video_unsupported",
            Self::FileInvalid => "mcp_file_invalid",
            Self::FileTooLarge => "mcp_file_too_large",
        }
    }

    const fn reason(self) -> &'static str {
        match self {
            Self::Binary => "is binary",
            Self::ImageUnsupported => "is not a PNG, JPEG, or WebP image",
            Self::ImageInvalid => "does not decode as its declared type",
            Self::ImageTooLarge => "exceeds the 5 MiB image limit",
            Self::VideoUnsupported => "is a video, which the reply cannot show",
            Self::FileInvalid => "is not valid base64 or has no file name or media type",
            Self::FileTooLarge => "exceeds the 50 MiB file limit",
        }
    }

    /// What the omission cost: a video or file never reaches the model, so
    /// leaving one out costs the reply.
    const fn outcome(self) -> &'static str {
        match self {
            Self::VideoUnsupported => "cannot be shown in the reply",
            Self::FileInvalid | Self::FileTooLarge => "cannot be attached to the reply",
            _ => "was not passed to the model",
        }
    }
}

/// One binary block as its note describes it.
pub(crate) struct Binary<'a> {
    pub(crate) what: &'a str,
    pub(crate) media_type: &'a str,
    /// Base64, as the protocol carries it.
    pub(crate) data: &'a str,
}

impl Binary<'_> {
    pub(crate) fn note(&self, omission: Omission) -> ContentBlock {
        ContentBlock::text(format!(
            "[{}: {} ({}, {} bytes) {} and {}]",
            omission.code(),
            self.what,
            self.media_type,
            decoded_len(self.data),
            omission.reason(),
            omission.outcome(),
        ))
    }
}
