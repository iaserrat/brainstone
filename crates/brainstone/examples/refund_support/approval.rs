use brainstone::runtime::{DurableStore, OperationId};
use std::{future::Future, time::Duration};

type Error = Box<dyn std::error::Error>;
const POLL: Duration = Duration::from_millis(25);

pub(crate) async fn execute<T>(
    store: &DurableStore,
    id: OperationId,
    work: impl Future<Output = Result<T, host::Error>>,
) -> Result<T, Error> {
    tokio::pin!(work);
    loop {
        tokio::select! {
            result = &mut work => return Ok(result?),
            () = tokio::time::sleep(POLL) => approve(store, id).await?,
        }
    }
}

async fn approve(store: &DurableStore, operation: OperationId) -> Result<(), Error> {
    for (id, owner) in store.pending_approvals().await? {
        if owner != operation.get() {
            continue;
        }
        let record = store.approval(&id).await?.ok_or("Approval disappeared")?;
        eprintln!(
            "Approval: {} {}",
            record.request.call.name, record.request.call.args
        );
        store
            .decide_approval(&id, true, "demo-cli-approve-flag")
            .await?;
    }
    Ok(())
}
