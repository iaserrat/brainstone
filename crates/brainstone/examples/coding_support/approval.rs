use brainstone::runtime::{DurableStore, OperationId};
use std::{future::Future, time::Duration};

const POLL: Duration = Duration::from_millis(25);
const MAX_DISPLAY_BYTES: usize = 256 * 1024;

pub(crate) async fn execute<T>(
    store: &DurableStore,
    operation: OperationId,
    demo: bool,
    work: impl Future<Output = Result<T, host::Error>>,
) -> Result<T, super::Error> {
    tokio::pin!(work);
    tokio::select! {
        result = &mut work => Ok(result?),
        error = operator(store, operation, demo) => Err(error),
        signal = tokio::signal::ctrl_c() => {
            signal?;
            Err("Interrupted; accepted work remains durable. Resume using its operation ID".into())
        }
    }
}
async fn operator(store: &DurableStore, operation: OperationId, demo: bool) -> super::Error {
    let console = (!demo).then(super::console::Console::new);
    loop {
        if let Err(error) = approve_pending(store, operation, console.as_ref()).await {
            return error;
        }
        tokio::time::sleep(POLL).await;
    }
}
async fn approve_pending(
    store: &DurableStore,
    operation: OperationId,
    console: Option<&super::console::Console>,
) -> Result<(), super::Error> {
    for (id, owner) in store.pending_approvals().await? {
        if owner != operation.get() {
            continue;
        }
        let record = store.approval(&id).await?.ok_or("Approval disappeared")?;
        let display = serde_json::to_string(&record.request)?;
        if display.len() > MAX_DISPLAY_BYTES {
            return Err("Approval exceeds the display limit; refusing to approve".into());
        }
        eprintln!("Approval: {display}");
        let approved = match console {
            Some(console) => console.decide().await?,
            None => super::demo::approves(&record.request.call),
        };
        store
            .decide_approval(&id, approved, "coding-example-operator")
            .await?;
    }
    Ok(())
}
