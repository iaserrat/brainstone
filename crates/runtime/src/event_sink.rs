use agent::{Event, Sink};

use crate::{Error, Lease, Store};

pub(crate) struct EventSink<'a, S> {
    store: &'a S,
    lease: &'a Lease,
}

impl<'a, S> EventSink<'a, S> {
    pub(crate) const fn new(store: &'a S, lease: &'a Lease) -> Self {
        Self { store, lease }
    }
}

impl<S: Store> Sink for EventSink<'_, S> {
    type Error = Error;

    async fn emit(&mut self, event: Event) -> Result<(), Self::Error> {
        self.store.record(self.lease, &event).await
    }
}
