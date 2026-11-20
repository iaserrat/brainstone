use std::io::Cursor;

use agent::{Image, ImageMediaType};
use image::{ImageFormat, ImageReader, Limits};

use crate::MediaError;

/// The largest encoded image accepted as model input: the strictest
/// provider's per-image limit.
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

/// Bounds the decode buffer, not the picture: a legitimate oversized image
/// decodes and is then downscaled, while a decompression bomb fails closed.
const MAX_ALLOC: u64 = 128 * 1024 * 1024;

/// # Errors
/// `Unsupported` for any media type but PNG, JPEG, or WebP.
pub fn media_type(value: &str) -> Result<ImageMediaType, MediaError> {
    match value {
        "image/jpeg" => Ok(ImageMediaType::Jpeg),
        "image/png" => Ok(ImageMediaType::Png),
        "image/webp" => Ok(ImageMediaType::Webp),
        _ => Err(MediaError::Unsupported),
    }
}

/// Validate `bytes` as the `expected` raster and fit it to the model's
/// resolution.
///
/// # Errors
/// `TooLarge` past [`MAX_IMAGE_BYTES`] or the decode bound, `Unsupported` for
/// another raster format, and `Invalid` for empty, corrupt, or mislabelled
/// bytes.
pub fn decode(bytes: Vec<u8>, expected: ImageMediaType) -> Result<Image, MediaError> {
    if bytes.is_empty() {
        return Err(MediaError::Invalid);
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(MediaError::TooLarge);
    }
    let mut reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| MediaError::Invalid)?;
    if format(reader.format())? != expected {
        return Err(MediaError::Invalid);
    }
    reader.limits(limits());
    let decoded = reader.decode().map_err(|error| image_error(&error))?;
    crate::fit::fit(&decoded, expected, bytes)
}

fn format(value: Option<ImageFormat>) -> Result<ImageMediaType, MediaError> {
    match value {
        Some(ImageFormat::Jpeg) => Ok(ImageMediaType::Jpeg),
        Some(ImageFormat::Png) => Ok(ImageMediaType::Png),
        Some(ImageFormat::WebP) => Ok(ImageMediaType::Webp),
        Some(_) => Err(MediaError::Unsupported),
        None => Err(MediaError::Invalid),
    }
}

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_ALLOC);
    limits
}

const fn image_error(error: &image::ImageError) -> MediaError {
    if matches!(error, image::ImageError::Limits(_)) {
        return MediaError::TooLarge;
    }
    MediaError::Invalid
}
