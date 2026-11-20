use std::fmt;

use serde::{Deserialize, Deserializer};

use super::{Image, ImageMediaType};

impl fmt::Debug for Image {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Image")
            .field("media_type", &self.media_type)
            .field("bytes", &"[REDACTED]")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

#[derive(Deserialize)]
struct Wire {
    media_type: ImageMediaType,
    #[serde(deserialize_with = "crate::bytes64::deserialize")]
    bytes: Vec<u8>,
    width: u32,
    height: u32,
}

pub(super) fn deserialize_image<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Image, D::Error> {
    let wire = Wire::deserialize(deserializer)?;
    Image::new(wire.media_type, wire.bytes, wire.width, wire.height)
        .map_err(serde::de::Error::custom)
}
