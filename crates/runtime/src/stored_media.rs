use agent::{File, Image};

/// Bytes a tool returned that a reply may show, stored under their
/// reference apart from the thread, so a reply resolves them however long
/// ago they were captured, by whichever lane of the conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Media {
    /// An image a reply may show.
    Image(Image),
    /// A file a reply may attach.
    File(File),
}

impl Media {
    /// The reference a reply names this by.
    #[must_use]
    pub fn reference(&self) -> String {
        match self {
            Self::Image(image) => media::reference(image),
            Self::File(file) => media::file_reference(file),
        }
    }
}

/// What one tool call left for the reply: every image it returned, which the
/// model saw too, and every file, which only the reply attaches.
pub(crate) fn of(images: &[Image], files: Vec<File>) -> Vec<Media> {
    let images = images.iter().cloned().map(Media::Image);
    images.chain(files.into_iter().map(Media::File)).collect()
}
