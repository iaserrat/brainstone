//! A `CreateFile` call's content as bytes, bounded before it is decoded.

use agent::ToolError;
use base64::Engine as _;
use serde::Deserialize;

/// How `content` carries the file's bytes.
#[derive(Deserialize, Default, Clone, Copy)]
pub(super) enum Encoding {
    /// The content is the file's text, stored as its UTF-8 bytes.
    #[default]
    #[serde(rename = "utf-8")]
    Utf8,
    /// The content is the file's bytes in standard base64.
    #[serde(rename = "base64")]
    Base64,
}

impl Encoding {
    /// What the content is when the call names no media type.
    pub(super) const fn media_type(self) -> &'static str {
        match self {
            Self::Utf8 => "text/plain",
            Self::Base64 => "application/octet-stream",
        }
    }

    pub(super) fn decode(self, content: String) -> Result<Vec<u8>, ToolError> {
        let size = match self {
            Self::Utf8 => content.len(),
            // Refuse oversized base64 before allocating its bytes.
            Self::Base64 => content.len() / 4 * 3,
        };
        if size > crate::MAX_FILE_BYTES {
            return Err(ToolError::new(
                "file_too_large",
                "file content exceeds the 50 MiB file limit",
            ));
        }
        match self {
            Self::Utf8 => Ok(content.into_bytes()),
            Self::Base64 => base64::engine::general_purpose::STANDARD
                .decode(content.trim())
                .map_err(|error| ToolError::new("invalid_file_content", error.to_string())),
        }
    }
}
