use agent::{Event, Message, Outcome};
use brainstone_runtime::{
    Accepted, Admitted, Claimed, Compaction, Error, Lease, Operation, OperationId, Prompt, Settled,
    Store,
};
use serde_json::Value;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Copy)]
pub(crate) enum Boundary {
    ToolResults,
    ModelAcknowledgement,
    Terminal,
    Heartbeat,
    SteeringAcknowledgement,
}

#[derive(Clone)]
pub(crate) struct FaultStore<S> {
    pub inner: S,
    boundary: Boundary,
    armed: Arc<AtomicBool>,
    // Cloned fault drivers share the claim that the completed attempt left behind.
    lease: Arc<Mutex<Option<Lease>>>,
    claim_delay: Duration,
}

impl<S> FaultStore<S> {
    pub(crate) fn new(inner: S, boundary: Boundary) -> Self {
        Self {
            inner,
            boundary,
            armed: Arc::new(AtomicBool::new(true)),
            lease: Arc::new(Mutex::new(None)),
            claim_delay: Duration::ZERO,
        }
    }
    pub(crate) fn with_claim_delay(mut self, delay: Duration) -> Self {
        self.claim_delay = delay;
        self
    }
    async fn claimed(&self, claim: Option<Claimed>) -> Option<Claimed> {
        if let Some(claim) = &claim {
            *self.lease.lock().unwrap() = Some(claim.lease.clone());
            if !self.claim_delay.is_zero() {
                tokio::time::sleep(self.claim_delay).await;
            }
        }
        claim
    }
    fn fire(&self, matches: bool) -> bool {
        matches && self.armed.swap(false, Ordering::SeqCst)
    }
}

fn outage() -> Error {
    Error::StoreUnavailable {
        message: "injected database outage".into(),
    }
}

impl<S: Store> Store for FaultStore<S> {
    async fn accept(&self, prompt: &Prompt) -> Result<Accepted, Error> {
        self.inner.accept(prompt).await
    }
    async fn admit_with_tools(
        &self,
        prompt: &Prompt,
        definitions: &[Value],
    ) -> Result<Admitted, Error> {
        self.inner.admit_with_tools(prompt, definitions).await
    }
    async fn operation(&self, id: OperationId) -> Result<Option<Operation>, Error> {
        self.inner.operation(id).await
    }
    async fn claim(&self, worker: &str, ttl: Duration) -> Result<Option<Claimed>, Error> {
        let claim = self.inner.claim(worker, ttl).await?;
        Ok(self.claimed(claim).await)
    }
    async fn claim_operation(
        &self,
        id: OperationId,
        worker: &str,
        ttl: Duration,
    ) -> Result<Option<Claimed>, Error> {
        let claim = self.inner.claim_operation(id, worker, ttl).await?;
        Ok(self.claimed(claim).await)
    }
    async fn record(&self, lease: &Lease, event: &Event) -> Result<(), Error> {
        if self.fire(
            matches!(self.boundary, Boundary::ToolResults)
                && matches!(
                    event,
                    Event::Appended {
                        message: Message::ToolResults(_)
                    }
                ),
        ) {
            return Err(outage());
        }
        self.inner.record(lease, event).await?;
        if self.fire(
            matches!(self.boundary, Boundary::ModelAcknowledgement)
                && matches!(
                    event,
                    Event::Appended {
                        message: Message::Assistant(_)
                    }
                ),
        ) {
            return Err(outage());
        }
        Ok(())
    }
    async fn save_media(
        &self,
        lease: &Lease,
        media: &[brainstone_runtime::Media],
    ) -> Result<(), Error> {
        self.inner.save_media(lease, media).await
    }
    async fn has_steering(&self, lease: &Lease) -> Result<Option<bool>, Error> {
        self.inner.has_steering(lease).await
    }
    async fn steering(&self, lease: &Lease) -> Result<Vec<Message>, Error> {
        let messages = self.inner.steering(lease).await?;
        if self.fire(
            matches!(self.boundary, Boundary::SteeringAcknowledgement) && !messages.is_empty(),
        ) {
            return Err(outage());
        }
        Ok(messages)
    }
    async fn settle(&self, lease: &Lease, outcome: &Outcome) -> Result<Settled, Error> {
        if self.fire(matches!(self.boundary, Boundary::Terminal)) {
            return Err(outage());
        }
        self.inner.settle(lease, outcome).await
    }
    async fn heartbeat(&self, lease: &Lease, ttl: Duration) -> Result<(), Error> {
        if self.fire(matches!(self.boundary, Boundary::Heartbeat)) {
            return Err(outage());
        }
        self.inner.heartbeat(lease, ttl).await
    }
    async fn compact(&self, compaction: &Compaction) -> Result<(), Error> {
        self.inner.compact(compaction).await
    }
}

impl<S: Store> FaultStore<S> {
    // The faulted driver has returned: no task can renew this captured claim.
    pub(crate) async fn expire_lease(&self) {
        let lease = self
            .lease
            .lock()
            .unwrap()
            .clone()
            .expect("driver claimed work");
        self.inner.heartbeat(&lease, Duration::ZERO).await.unwrap();
    }
}
