use runtime::{OperationId, Prompt};

use crate::{Error, Host};

/// Durable prompt admission used by surface adapters.
///
/// The returned operation id is the receipt for synchronous surfaces; event
/// surfaces may ignore it after acknowledging their external event.
pub trait Admission: Clone + Send + Sync {
    /// Rejects the actor before anything is stored; the default admits everyone.
    fn preflight(&self, _actor: Option<&str>) -> impl Future<Output = Result<(), Error>> + Send {
        async { Ok(()) }
    }

    /// Stores the prompt durably and returns the operation id.
    fn accept(&self, prompt: &Prompt) -> impl Future<Output = Result<OperationId, Error>> + Send;
}

impl<S, M> Admission for Host<S, M>
where
    S: runtime::Store,
    M: agent::Model,
{
    async fn accept(&self, prompt: &Prompt) -> Result<OperationId, Error> {
        Host::admit(self, prompt)
            .await
            .map(|admitted| admitted.operation().id)
    }
}
