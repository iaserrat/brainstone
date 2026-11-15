use base64::Engine as _;
use serde::{Deserialize, Deserializer, Serialize};

#[path = "image_data.rs"]
mod data;
use std::{error, fmt};
/// The raster types every supported provider accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageMediaType {
    #[serde(rename = "image/jpeg")]
    /// JPEG.
    Jpeg,
    #[serde(rename = "image/png")]
    /// PNG.
    Png,
    #[serde(rename = "image/webp")]
    /// WebP.
    Webp,
}

impl ImageMediaType {
    #[must_use]
    /// The media type as providers spell it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
        }
    }
}
/// One image the model can read.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct Image {
    media_type: ImageMediaType,
    #[serde(with = "crate::bytes64")]
    bytes: Vec<u8>,
    width: u32,
    height: u32,
}

impl Image {
    /// # Errors
    /// Returns `invalid_image` for empty bytes or zero dimensions.
    pub fn new(
        media_type: ImageMediaType,
        bytes: Vec<u8>,
        width: u32,
        height: u32,
    ) -> Result<Self, ImageError> {
        if bytes.is_empty() || width == 0 || height == 0 {
            return Err(ImageError);
        }
        Ok(Self {
            media_type,
            bytes,
            width,
            height,
        })
    }

    #[must_use]
    /// The image's media type.
    pub const fn media_type(&self) -> ImageMediaType {
        self.media_type
    }
    #[must_use]
    /// The encoded image bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    #[must_use]
    /// Pixel width, as the producer declared it.
    pub const fn width(&self) -> u32 {
        self.width
    }
    #[must_use]
    /// Pixel height, as the producer declared it.
    pub const fn height(&self) -> u32 {
        self.height
    }
    #[must_use]
    /// The bytes encoded as standard base64, for provider payloads.
    pub fn base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(&self.bytes)
    }
}

/// The image bytes or dimensions were unusable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageError;

impl ImageError {
    #[must_use]
    /// The stable code a caller branches on.
    pub const fn code(self) -> &'static str {
        "invalid_image"
    }
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("image bytes and dimensions must be non-zero")
    }
}
impl error::Error for ImageError {}

impl<'de> Deserialize<'de> for Image {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        data::deserialize_image(deserializer)
    }
}
