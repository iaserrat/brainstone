use agent::{Image, ImageMediaType};
use image::DynamicImage;

use crate::MediaError;

/// The provider's vision pipeline downsamples anything past this long edge,
/// so resizing here loses nothing the model would have seen while keeping
/// oversized captures (a full-page screenshot easily passes 9,000px) inside
/// every provider limit.
const MAX_LONG_EDGE: u32 = 1_568;

/// Pass an image within the model resolution through untouched; shrink an
/// oversized one to fit and re-encode it as PNG.
pub(crate) fn fit(
    decoded: &DynamicImage,
    media_type: ImageMediaType,
    bytes: Vec<u8>,
) -> Result<Image, MediaError> {
    if decoded.width().max(decoded.height()) <= MAX_LONG_EDGE {
        return Image::new(media_type, bytes, decoded.width(), decoded.height())
            .map_err(|_| MediaError::Invalid);
    }
    let resized = decoded.thumbnail(MAX_LONG_EDGE, MAX_LONG_EDGE);
    let png = crate::encode::encode_png(&resized)?;
    Image::new(ImageMediaType::Png, png, resized.width(), resized.height())
        .map_err(|_| MediaError::Invalid)
}
