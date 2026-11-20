//! One `tool_media` row, written from and read back as the media it holds.

use agent::{File, Image};

use crate::{Error, Media};

/// `media_type`, `bytes`, `width`, `height`, `name`.
pub(super) type Row = (String, Vec<u8>, Option<i32>, Option<i32>, Option<String>);

/// `media_type`, `bytes`, the image's `(width, height)`, and the file's `name`.
type Columns<'a> = (&'a str, &'a [u8], Option<(i32, i32)>, Option<&'a str>);

pub(super) fn columns(item: &Media) -> Result<Columns<'_>, Error> {
    Ok(match item {
        Media::Image(image) => {
            let size = Some(dimensions(image)?);
            (image.media_type().as_str(), image.bytes(), size, None)
        }
        Media::File(file) => (file.media_type(), file.bytes(), None, Some(file.name())),
    })
}

fn dimensions(image: &Image) -> Result<(i32, i32), Error> {
    let width = i32::try_from(image.width()).map_err(Error::corrupt)?;
    let height = i32::try_from(image.height()).map_err(Error::corrupt)?;
    Ok((width, height))
}

/// A stored record this build cannot interpret is rejected, never guessed.
/// Only a file has a name, so the name, not the media type, marks one.
pub(super) fn parse((media_type, bytes, width, height, name): Row) -> Result<Media, Error> {
    if let Some(name) = name {
        return File::new(name, &media_type, bytes)
            .map(Media::File)
            .map_err(Error::corrupt);
    }
    let kind = media::media_type(&media_type).map_err(Error::corrupt)?;
    let (Some(width), Some(height)) = (width, height) else {
        return Err(Error::corrupt("image without dimensions"));
    };
    let width = u32::try_from(width).map_err(Error::corrupt)?;
    let height = u32::try_from(height).map_err(Error::corrupt)?;
    Image::new(kind, bytes, width, height)
        .map(Media::Image)
        .map_err(Error::corrupt)
}
