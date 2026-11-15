/// A file one thread message carries, as its surface described it. Every
/// field but the id is untrusted surface metadata: planning uses it to bound
/// network work, and the bytes are validated again after download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRef {
    /// The surface's stable id for the file.
    pub id: String,
    /// The user-facing file name, when the surface reports one.
    pub name: Option<String>,
    /// The declared MIME type, `None` until the surface has resolved it.
    pub media_type: Option<String>,
    /// The declared size in bytes, `None` until the surface has resolved it.
    pub size: Option<u64>,
    /// Where the surface downloads the bytes from.
    pub url: Option<String>,
}

impl FileRef {
    /// Metadata a surface may return partially; an incomplete file is
    /// described once before planning.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.media_type.is_some() && self.size.is_some() && self.url.is_some()
    }

    /// The user-facing file name when the surface provided one, else the id,
    /// so every skip note and inline document is attributable.
    #[must_use]
    pub fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.id.clone())
    }
}
