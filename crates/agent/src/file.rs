use std::fmt;

#[path = "file_error.rs"]
mod error;
pub use error::FileError;

/// The longest file name, in bytes, that every common filesystem keeps as given.
pub const MAX_FILE_NAME_BYTES: usize = 255;

/// A file a tool made or returned for the reply to attach. It never joins the
/// thread: a durable driver stores it apart and the model reads only its
/// reference.
#[derive(Clone, PartialEq, Eq)]
pub struct File {
    name: String,
    media_type: String,
    bytes: Vec<u8>,
}

impl File {
    /// `media_type` keeps only its lowercase `type/subtype`.
    ///
    /// # Errors
    /// `invalid_file_name` for a name a person could not save as given,
    /// `invalid_media_type` for one that is not `type/subtype`, and
    /// `empty_file` for empty bytes.
    pub fn new(
        name: impl Into<String>,
        media_type: &str,
        bytes: Vec<u8>,
    ) -> Result<Self, FileError> {
        let name = name.into();
        if !saveable(&name) {
            return Err(FileError::Name);
        }
        let essence = media_type.split(';').next().unwrap_or_default().trim();
        let media_type = essence.to_ascii_lowercase();
        if !plain_media_type(&media_type) {
            return Err(FileError::MediaType);
        }
        if bytes.is_empty() {
            return Err(FileError::Empty);
        }
        Ok(Self {
            name,
            media_type,
            bytes,
        })
    }

    #[must_use]
    /// The file name the reply attaches it under.
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    /// The file's `type/subtype` media type.
    pub fn media_type(&self) -> &str {
        &self.media_type
    }
    #[must_use]
    /// The file's bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// One path segment, kept exactly: no separator, no control character, no
/// padding a surface would trim, and not a directory reference.
fn saveable(name: &str) -> bool {
    let bounded = !name.is_empty() && name.len() <= MAX_FILE_NAME_BYTES;
    let segment = !matches!(name, "." | "..") && name.trim() == name;
    bounded && segment && !name.chars().any(forbidden)
}

fn forbidden(character: char) -> bool {
    character.is_control() || matches!(character, '/' | '\\')
}

/// `type/subtype`, each made of RFC 6838 restricted-name characters.
fn plain_media_type(media_type: &str) -> bool {
    let Some((kind, subtype)) = media_type.split_once('/') else {
        return false;
    };
    restricted_name(kind) && restricted_name(subtype)
}

fn restricted_name(part: &str) -> bool {
    let allowed = |byte: u8| byte.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&byte);
    !part.is_empty() && part.bytes().all(allowed)
}

impl fmt::Debug for File {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("File")
            .field("name", &self.name)
            .field("media_type", &self.media_type)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}
