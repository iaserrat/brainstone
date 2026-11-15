use std::io::Cursor;

use agent::Image;
use image::{DynamicImage, ImageFormat};

use crate::MediaError;

/// `image` as PNG bytes, for a destination that does not accept its format.
///
/// # Errors
/// `Invalid` when the stored bytes no longer decode.
pub fn png(image: &Image) -> Result<Vec<u8>, MediaError> {
    let decoded = image::load_from_memory(image.bytes()).map_err(|_| MediaError::Invalid)?;
    encode_png(&decoded)
}

pub(crate) fn encode_png(image: &DynamicImage) -> Result<Vec<u8>, MediaError> {
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|_| MediaError::Invalid)?;
    Ok(out.into_inner())
}
