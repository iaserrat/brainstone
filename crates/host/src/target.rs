use agent::Model;
use runtime::{Operation, OperationId, OperationState, Store};

use crate::{Error, Host};

impl<S: Store, M: Model> Host<S, M> {
    /// Drive only `id`, waiting when another worker owns its session.
    ///
    /// # Errors
    /// Returns a stable host or runtime failure, including shutdown.
    pub async fn execute(&self, id: OperationId) -> Result<Operation, Error> {
        loop {
            let accepting = self.inner.accepting.read().await;
            if !*accepting {
                return Err(Error::ShuttingDown);
            }
            let ran = self
                .inner
                .runtime
                .run_operation(id, &self.inner.model, &self.inner.tools)
                .await?;
            crate::operation_log::record(ran.as_ref());
            let operation = match ran {
                Some(operation) => operation,
                None => self.operation(id).await?,
            };
            drop(accepting);
            if terminal(&operation) {
                return Ok(operation);
            }
            tokio::time::sleep(self.inner.config.poll_interval).await;
        }
    }

    async fn operation(&self, id: OperationId) -> Result<Operation, Error> {
        self.inner.runtime.operation(id).await?.ok_or_else(|| {
            runtime::Error::CorruptRecord {
                message: format!("operation {} disappeared", id.get()),
            }
            .into()
        })
    }
}

const fn terminal(operation: &Operation) -> bool {
    matches!(
        operation.state,
        OperationState::Completed | OperationState::Failed
    )
}
