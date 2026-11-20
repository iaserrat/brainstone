use agent::Registry;
use host::Host;
use runtime::{DurableStore, Runtime};

use crate::configured::service_admission::AgentHost;
use crate::configured::service_model::FailoverModel;
use crate::configured::{CoreSettings, Error};

pub(crate) struct Parts {
    pub(crate) store: DurableStore,
    model: FailoverModel,
    runtime: Runtime<DurableStore>,
}

pub(crate) async fn prepare(settings: &CoreSettings) -> Result<Parts, Error> {
    let store = DurableStore::connect(&settings.database_url)
        .await
        .map_err(|error| Error::component(error.code(), error))?;
    let model = crate::configured::service_model::build(settings).await?;
    let runtime = crate::configured::service_policy::runtime(settings, store.clone())?;
    Ok(Parts {
        store,
        model,
        runtime,
    })
}

pub(crate) fn host(
    settings: &CoreSettings,
    parts: Parts,
    tools: Registry,
) -> Result<AgentHost, Error> {
    let tools =
        crate::configured::service_subagents::tools(settings, &parts.runtime, &parts.model, tools)?;
    Ok(Host::new(parts.runtime, parts.model, tools))
}
