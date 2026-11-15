use runtime::{AdmissionQuota, LaneStore, OperationId, Prompt};

use crate::{Admission, Error};

/// Pre-acceptance admission control: reject work before any durable write or
/// model spend. Both checks default to off, leaving admission untouched.
#[derive(Clone, Debug)]
pub struct GatedAdmission<A, S> {
    inner: A,
    store: S,
    allowed_actors: Option<Vec<String>>,
    actor_daily_limit: Option<usize>,
}

impl<A, S> GatedAdmission<A, S> {
    #[must_use]
    /// Wraps `inner`, refusing actors outside `allowed_actors` and above
    /// `actor_daily_limit` operations per day.
    pub const fn new(
        inner: A,
        store: S,
        allowed_actors: Option<Vec<String>>,
        actor_daily_limit: Option<usize>,
    ) -> Self {
        Self {
            inner,
            store,
            allowed_actors,
            actor_daily_limit,
        }
    }
}

impl<A: Admission, S: AdmissionQuota + LaneStore> Admission for GatedAdmission<A, S> {
    async fn preflight(&self, actor: Option<&str>) -> Result<(), Error> {
        if let Some(allowed) = &self.allowed_actors
            && !actor.is_some_and(|actor| allowed.iter().any(|entry| entry == actor))
        {
            return Err(Error::AdmissionDenied);
        }
        if let (Some(limit), Some(actor)) = (self.actor_daily_limit, actor)
            && self.store.actor_operations_last_day(actor).await? >= limit
        {
            return Err(Error::AdmissionQuotaExceeded);
        }
        self.inner.preflight(actor).await
    }

    /// With an allowlist, a prompt without an actor or with an unlisted one
    /// is `admission_denied`. With a limit, an actor at or past it is
    /// `admission_quota_exceeded`; a prompt without an actor has nothing to
    /// count and passes, which the allowlist exists to prevent.
    async fn accept(&self, prompt: &Prompt) -> Result<OperationId, Error> {
        let existing = self
            .store
            .request_exists(prompt.address(), prompt.request_id())
            .await?;
        if !existing
            && let Err(error) = self.preflight(prompt.actor()).await
            && !self
                .store
                .request_exists(prompt.address(), prompt.request_id())
                .await?
        {
            return Err(error);
        }
        self.inner.accept(prompt).await
    }
}
