use super::Execution;
use crate::event_sink::EventSink;
use crate::{Error, Store};
use agent::Model;
use std::time::Duration;
const STEERING_POLL: Duration = Duration::from_millis(100);

impl<S: Store, M: Model> Execution<'_, S, M> {
    pub(super) async fn call_model(&mut self) -> Result<Option<agent::ModelResponse>, Error> {
        let response = {
            let mut sink = EventSink::new(self.store, &self.claimed.lease);
            tokio::select! {
                biased;
                pending = pending(self.store, &self.claimed.lease) => { pending?; None }
                response = agent::observation::generate_stream(self.model, &self.claimed.thread, &self.definitions, &mut sink) => Some(response?),
            }
        };
        // Consume only after generation has been dropped: cancelling a consuming
        // database future could commit steering without returning it to this thread.
        if self.apply_steering().await? {
            return Ok(None);
        }
        Ok(response)
    }
}

async fn pending(store: &impl Store, lease: &crate::Lease) -> Result<(), Error> {
    loop {
        tokio::time::sleep(STEERING_POLL).await;
        if store.has_steering(lease).await? == Some(true) {
            return Ok(());
        }
    }
}
