use agent::Model;
use runtime::Store;

use crate::{Error, Host};

impl<S: Store, M: Model> Host<S, M> {
    /// Poll and execute durable work until shutdown begins.
    ///
    /// # Errors
    ///
    /// Returns the runtime error that stopped a worker: `store_unavailable` or
    /// `lease_lost` once a worker exhausts its retry pauses, any other at once.
    pub async fn run(&self) -> Result<(), Error>
    where
        S: 'static,
        M: 'static,
    {
        let mut workers = tokio::task::JoinSet::new();
        for _ in 0..self.inner.config.workers {
            let worker = self.clone();
            workers.spawn(async move { worker.run_loop().await });
        }
        while let Some(result) = workers.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => return self.fail(&mut workers, error).await,
                Err(error) => {
                    let error = Error::WorkerFailed {
                        message: error.to_string(),
                    };
                    return self.fail(&mut workers, error).await;
                }
            }
        }
        Ok(())
    }

    async fn fail(
        &self,
        workers: &mut tokio::task::JoinSet<Result<(), Error>>,
        error: Error,
    ) -> Result<(), Error> {
        self.shutdown().await;
        workers.abort_all();
        while workers.join_next().await.is_some() {}
        Err(error)
    }

    async fn run_loop(&self) -> Result<(), Error> {
        loop {
            let notified = self.inner.wake.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            match self.attempt().await? {
                Cycle::Closed => return Ok(()),
                Cycle::Worked => {}
                Cycle::Idle => {
                    tokio::select! {
                        () = &mut notified => {}
                        () = tokio::time::sleep(self.inner.config.poll_interval) => {}
                    }
                }
            }
        }
    }

    pub(crate) async fn cycle(&self) -> Result<Cycle, Error> {
        let accepting = self.inner.accepting.read().await;
        if !*accepting {
            return Ok(Cycle::Closed);
        }
        let operation = self
            .inner
            .runtime
            .run_next(&self.inner.model, &self.inner.tools)
            .await?;
        crate::operation_log::record(operation.as_ref());
        Ok(operation.map_or(Cycle::Idle, |_| Cycle::Worked))
    }
}

pub(crate) enum Cycle {
    Closed,
    Worked,
    Idle,
}
