use std::time::Duration;

use runtime::{DurableStore, Runtime};

use crate::configured::{CoreSettings, Error};

const LEASE_TTL: Duration = Duration::from_secs(30);

pub(crate) fn runtime(
    settings: &CoreSettings,
    store: DurableStore,
) -> Result<Runtime<DurableStore>, Error> {
    let compaction = runtime::CompactionPolicy::new(
        settings.auto_compact_token_limit,
        policy::COMPACTION_PROMPT,
    )
    .map_err(|error| Error::component(error.code(), error))?;
    let mut config = runtime::Config::new(&settings.worker_id, LEASE_TTL)
        .map_err(|error| Error::component(error.code(), error))?;
    if let Some(policy) = &settings.tool_policy {
        config = config.with_tool_policy((**policy).clone());
    }
    Ok(Runtime::with_config(
        store,
        config.with_compaction(compaction),
    ))
}
