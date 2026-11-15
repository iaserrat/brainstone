//! An MCP image block, validated and fitted exactly as a surface's image
//! attachment is, so the model receives what it would from a person.

use agent::Image;
use base64::Engine as _;
use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use media::MediaError;
use rmcp::model::ContentBlock;

use crate::tool_content_note::{Binary, Omission};

/// The block that replaces an image: its attachment document when the image
/// joins `images`, or the note saying why it did not.
pub(crate) fn read(data: &str, media_type: &str, images: &mut Vec<Image>) -> ContentBlock {
    match decode(data, media_type) {
        Ok(image) => {
            let block = ContentBlock::text(media::attachment(&image).to_string());
            images.push(image);
            block
        }
        Err(omission) => Binary {
            what: "image",
            media_type,
            data,
        }
        .note(omission),
    }
}

/// Servers differ on padding, and some send a data URL; both carry the same
/// bytes, so neither is a reason to keep an image from the model.
pub(crate) const BASE64: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

fn decode(data: &str, media_type: &str) -> Result<Image, Omission> {
    let expected = media::media_type(media_type).map_err(omission)?;
    let data = payload(data);
    // Refuse an oversized image before allocating its bytes.
    if decoded_len(data) > media::MAX_IMAGE_BYTES {
        return Err(Omission::ImageTooLarge);
    }
    let bytes = BASE64.decode(data).map_err(|_| Omission::ImageInvalid)?;
    media::decode(bytes, expected).map_err(omission)
}

/// The base64 after a `data:<type>;base64,` prefix, or `data` itself.
pub(crate) fn payload(data: &str) -> &str {
    data.strip_prefix("data:")
        .and_then(|url| url.split_once(";base64,"))
        .map_or(data, |(_, payload)| payload)
}

const fn omission(error: MediaError) -> Omission {
    match error {
        MediaError::Unsupported => Omission::ImageUnsupported,
        MediaError::Invalid => Omission::ImageInvalid,
        MediaError::TooLarge => Omission::ImageTooLarge,
    }
}

/// The byte length base64 `data` decodes to, without decoding it.
pub(crate) fn decoded_len(data: &str) -> usize {
    let padding = data.bytes().rev().take_while(|byte| *byte == b'=').count();
    (data.len() / 4 * 3).saturating_sub(padding)
}
