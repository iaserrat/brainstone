use std::{error, fmt};

/// Why bytes could not become a [`File`](super::File).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FileError {
    /// The name is not one safe path segment.
    Name,
    /// The media type is not `type/subtype`.
    MediaType,
    /// The file has no bytes.
    Empty,
}

impl FileError {
    #[must_use]
    /// The stable code a caller branches on.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Name => "invalid_file_name",
            Self::MediaType => "invalid_media_type",
            Self::Empty => "empty_file",
        }
    }
}

impl fmt::Display for FileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Name => {
                "a file name must be one path segment of 1 to 255 bytes, \
                 with no control characters or surrounding spaces"
            }
            Self::MediaType => "a media type must be type/subtype",
            Self::Empty => "a file must hold at least one byte",
        })
    }
}

impl error::Error for FileError {}
