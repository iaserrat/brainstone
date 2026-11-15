//! The tool registry: memory, silence, skills, schedules, files.

use std::sync::Arc;

use agent::Registry;
use runtime::DurableStore;
use surface::Destinations;

use crate::{CoreSettings, Settings};

/// Every tool the engine offers. `destinations` decides where schedule runs
/// post; without one the schedule tools are not offered.
pub(crate) fn tools<D: Destinations + 'static>(
    settings: &Settings,
    store: &DurableStore,
    base: &Registry,
    destinations: Option<D>,
) -> Registry {
    let tools = optional_tools(&settings.core, store, base);
    let tools = file_tools(tools, settings.file_tool);
    match destinations {
        Some(destinations) => policy::registry(
            store.clone(),
            &tools,
            settings.core.schedule_min_interval_minutes,
            destinations,
        ),
        None => tools,
    }
}

/// `attaches`: the consumer saves its reply's attachments, so it offers `CreateFile`.
fn file_tools(tools: Registry, attaches: bool) -> Registry {
    if !attaches {
        return tools;
    }
    tools.extend(vec![Arc::new(media::CreateFile)])
}

fn optional_tools(settings: &CoreSettings, store: &DurableStore, base: &Registry) -> Registry {
    let mut tools = memory::tools(&settings.memory, store);
    if settings.tools.silence {
        tools.push(Arc::new(policy::Silence));
    }
    settings.skills.registry(base.clone()).extend(tools)
}
