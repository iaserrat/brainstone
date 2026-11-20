use agent::{Image, ImageMediaType};
use media::MediaError;

use crate::{AttachmentError, FileRef};

pub(crate) fn decode(file: &FileRef, bytes: Vec<u8>) -> Result<Image, AttachmentError> {
    let expected = media_type(file.media_type.as_deref().unwrap_or_default())?;
    media::decode(bytes, expected).map_err(attachment_error)
}

pub(crate) fn media_type(value: &str) -> Result<ImageMediaType, AttachmentError> {
    media::media_type(value).map_err(attachment_error)
}

const fn attachment_error(error: MediaError) -> AttachmentError {
    match error {
        MediaError::Unsupported => AttachmentError::Unsupported,
        MediaError::Invalid => AttachmentError::Invalid,
        MediaError::TooLarge => AttachmentError::TooLarge,
    }
}
