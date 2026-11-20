use crate::memory_backend::memory_state::State;
use crate::{Error, Lease, Media, OperationId};

impl State {
    pub(crate) fn save_media(&mut self, lease: &Lease, media: &[Media]) -> Result<(), Error> {
        let root = self.root_session(lease.operation_id)?;
        for item in media {
            self.media.insert((root, item.reference()), item.clone());
        }
        Ok(())
    }

    pub(crate) fn media(&self, operation: OperationId, reference: &str) -> Option<Media> {
        let root = self.root_session(operation).ok()?;
        self.media.get(&(root, reference.to_owned())).cloned()
    }
}
