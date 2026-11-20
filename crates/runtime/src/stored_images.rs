use crate::Error;
use agent::{Image, ImageMediaType};

pub(crate) type Row = (i64, i16, String, Vec<u8>, i32, i32);

pub(crate) fn parse_group(rows: Vec<Row>, owner: i64, expected: i16) -> Result<Vec<Image>, Error> {
    if rows.len() != usize::try_from(expected).map_err(Error::corrupt)? {
        return Err(corrupt("image count mismatch"));
    }
    rows.into_iter()
        .enumerate()
        .map(|(expected, row)| {
            let ordinal = usize::try_from(row.1).map_err(Error::corrupt)?;
            if row.0 != owner || ordinal != expected {
                return Err(corrupt("image ordinal gap"));
            }
            let media = media_type(&row.2)?;
            let width = u32::try_from(row.4).map_err(Error::corrupt)?;
            let height = u32::try_from(row.5).map_err(Error::corrupt)?;
            Image::new(media, row.3, width, height).map_err(Error::corrupt)
        })
        .collect()
}

fn media_type(value: &str) -> Result<ImageMediaType, Error> {
    match value {
        "image/jpeg" => Ok(ImageMediaType::Jpeg),
        "image/png" => Ok(ImageMediaType::Png),
        "image/webp" => Ok(ImageMediaType::Webp),
        _ => Err(corrupt("unknown image media type")),
    }
}

fn corrupt(message: &str) -> Error {
    Error::CorruptRecord {
        message: message.to_owned(),
    }
}
