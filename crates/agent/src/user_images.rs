use serde::{Deserialize, Deserializer, Serialize};

use crate::{Image, ImageError};

/// User text plus the images it arrived with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserImages {
    text: String,
    images: Vec<Image>,
}

impl UserImages {
    /// # Errors
    /// Returns `invalid_image` when `images` is empty.
    pub fn new(text: impl Into<String>, images: Vec<Image>) -> Result<Self, ImageError> {
        if images.is_empty() {
            return Err(ImageError);
        }
        Ok(Self {
            text: text.into(),
            images,
        })
    }

    #[must_use]
    /// The text part.
    pub fn text(&self) -> &str {
        &self.text
    }
    #[must_use]
    /// The images, in the order they were sent.
    pub fn images(&self) -> &[Image] {
        &self.images
    }
}

#[derive(Deserialize)]
struct Wire {
    text: String,
    images: Vec<Image>,
}

impl<'de> Deserialize<'de> for UserImages {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.text, wire.images).map_err(serde::de::Error::custom)
    }
}
