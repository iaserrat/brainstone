use std::time::Duration;

use agent::{Model, Registry};

use crate::execution::Execution;
use crate::{Claimed, CompactionPolicy, Error, Lease, Store};

pub(crate) struct Effects<'a, M> {
    pub(crate) model: &'a M,
    pub(crate) tools: &'a Registry,
    pub(crate) compaction: Option<&'a CompactionPolicy>,
    pub(crate) policy: Option<&'a crate::ToolPolicy>,
}

pub(crate) async fn drive<S: Store, M: Model>(
    store: &S,
    claimed: &mut Claimed,
    ttl: Duration,
    effects: Effects<'_, M>,
) -> Result<(), Error> {
    let lease = claimed.lease.clone();
    // Listing runs under the heartbeat: a slow tool source cannot cost the lease.
    let run = async {
        let grant = claimed.tool_grant.as_deref();
        let tools = effects.tools.listed().await.select(grant, &[]);
        Execution::new(store, claimed, &effects, tools).run().await
    };
    tokio::pin!(run);
    let interval = ttl / 3;
    loop {
        tokio::select! {
            biased;
            result = &mut run => return result,
            result = heartbeat(store, &lease, ttl, interval) => result?,
        }
    }
}

/// Renews the lease as a select branch so `run` keeps being polled while the
/// renewal is in flight. The postgres store locks the session row for every
/// append transaction and the renewal updates that same row; awaiting the
/// renewal in the branch body deadlocked a live operation mid-append.
async fn heartbeat<S: Store>(
    store: &S,
    lease: &Lease,
    ttl: Duration,
    interval: Duration,
) -> Result<(), Error> {
    tokio::time::sleep(interval).await;
    store.heartbeat(lease, ttl).await
}
