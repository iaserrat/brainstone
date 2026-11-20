//! An in-memory surface: files keyed by URL, metadata keyed by id, and a log
//! of every download so a test can prove none happened.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{AttachmentError, Fetched, FileRef, ThreadHistory, ThreadSource};

/// The fake's systemic failure: transport, authentication, rate limits.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Down;

impl From<Down> for agent::ToolError {
    fn from(_: Down) -> Self {
        Self::new("surface_down", "the surface is down")
    }
}

#[derive(Default)]
pub(crate) struct Fake {
    pub(crate) bodies: HashMap<String, Vec<u8>>,
    pub(crate) described: HashMap<String, FileRef>,
    /// Shared so a test can keep a handle after a tool takes the source.
    pub(crate) downloads: Arc<Mutex<Vec<String>>>,
    pub(crate) down: bool,
    pub(crate) history: Option<ThreadHistory>,
}

impl Fake {
    pub(crate) fn serving(files: &[(&FileRef, &[u8])]) -> Self {
        let bodies = files
            .iter()
            .map(|(file, body)| (file.url.clone().unwrap(), body.to_vec()))
            .collect();
        Self {
            bodies,
            ..Self::default()
        }
    }
}

pub(crate) fn file(id: &str, name: &str, mime: &str, size: usize) -> FileRef {
    FileRef {
        id: id.to_owned(),
        name: Some(name.to_owned()),
        media_type: Some(mime.to_owned()),
        size: Some(u64::try_from(size).unwrap()),
        url: Some(format!("https://files.example/{id}")),
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "Preserve execution on poll across the async trait seam."
)]
impl ThreadSource for Fake {
    type Error = Down;

    async fn messages(
        &self,
        _target: &runtime::DeliveryTarget,
    ) -> Result<ThreadHistory, Self::Error> {
        self.history.clone().ok_or(Down)
    }

    async fn describe(&self, file: &FileRef) -> Result<Fetched<FileRef>, Self::Error> {
        let described = self.described.get(&file.id).cloned();
        Ok(described.ok_or(AttachmentError::Unavailable))
    }

    async fn bytes(&self, file: &FileRef, cap: usize) -> Result<Fetched<Vec<u8>>, Self::Error> {
        if self.down {
            return Err(Down);
        }
        let url = file.url.clone().unwrap_or_default();
        self.downloads.lock().unwrap().push(url.clone());
        Ok(match self.bodies.get(&url) {
            None => Err(AttachmentError::Unavailable),
            Some(body) if body.len() > cap => Err(AttachmentError::TooLarge),
            Some(body) => Ok(body.clone()),
        })
    }
}
