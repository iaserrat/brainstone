use agent::Registry;
use runtime::{DurableStore, Runtime};

use crate::service_model::FailoverModel;
use crate::{CoreSettings, Error};

pub(crate) fn tools(
    settings: &CoreSettings,
    runtime: &Runtime<DurableStore>,
    model: &FailoverModel,
    base: Registry,
) -> Result<Registry, Error> {
    if !settings.model_capabilities().tools {
        return Ok(Registry::default());
    }
    let control = subagents::Control::new(
        runtime.clone(),
        settings.subagents.clone(),
        base,
        model.clone(),
    );
    control
        .validate()
        .map_err(|error| Error::component(error.code(), error))?;
    Ok(control.registry())
}
