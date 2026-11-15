use host::Host;
use runtime::DurableStore;

use crate::{CoreSettings, service_model::FailoverModel};

type ServiceModel = FailoverModel;
pub(crate) type AgentHost = Host<DurableStore, ServiceModel>;
pub(crate) type MemoryHost = memory::MemoryAdmission<AgentHost, DurableStore>;
pub(crate) type SelectedHost =
    host::ModelSelection<skills::SkillAdmission<MemoryHost>, DurableStore>;
pub(crate) type GatedHost = host::GatedAdmission<SelectedHost, DurableStore>;

pub(crate) fn chain(settings: &CoreSettings, host: &AgentHost, store: &DurableStore) -> GatedHost {
    let memories = memory::MemoryAdmission::new(
        host.clone(),
        store.clone(),
        settings.memory.scope,
        settings.memory.inject,
    );
    let skills = skills::SkillAdmission::new(memories, settings.skills.clone());
    let selected = host::ModelSelection::new(
        skills,
        store.clone(),
        settings.admission.model_selector.clone(),
    );
    host::GatedAdmission::new(
        selected,
        store.clone(),
        settings.admission.allowed_actors.clone(),
        settings.admission.actor_daily_limit,
    )
}
