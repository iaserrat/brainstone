use std::{error, fmt};

/// Why bytes did not become model input. Callers map each variant to their
/// own stable code, so a variant here is a branch, not a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaError {
    /// Not a PNG, JPEG, or WebP raster.
    Unsupported,
    /// Empty, corrupt, or contradicting its declared type.
    Invalid,
    /// Past the per-image byte cap, or a decode too large to buffer.
    TooLarge,
}

impl fmt::Display for MediaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unsupported => "only JPEG, PNG, or WebP images are supported",
            Self::Invalid => "the image is invalid or contradicts its declared type",
            Self::TooLarge => "the image exceeds the supported size",
        })
    }
}

impl error::Error for MediaError {}
