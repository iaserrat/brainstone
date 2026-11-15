use std::sync::Arc;

use agent::{Model, Registry, Tool};
use runtime::{Runtime, Store, SubagentStore};

use crate::Catalog;

const MAX_DEPTH: u8 = 3;

/// The state a sub-agent tool needs: the runtime, the catalog, and the model.
#[derive(Debug)]
pub struct Control<S, M> {
    pub(crate) runtime: Runtime<S>,
    pub(crate) catalog: Catalog,
    pub(crate) base: Registry,
    pub(crate) max_depth: u8,
    pub(crate) model: M,
    pub(crate) allow_background: bool,
}

impl<S: Store + SubagentStore + 'static, M: Model + 'static> Control<S, M> {
    /// Controls for a host that drains work in the background.
    #[must_use]
    pub fn new(runtime: Runtime<S>, catalog: Catalog, base: Registry, model: M) -> Arc<Self> {
        Arc::new(Self {
            runtime,
            catalog,
            base,
            max_depth: MAX_DEPTH,
            model,
            allow_background: true,
        })
    }

    /// Build controls for a one-shot host that has no background worker.
    #[must_use]
    pub fn foreground(
        runtime: Runtime<S>,
        catalog: Catalog,
        base: Registry,
        model: M,
    ) -> Arc<Self> {
        Arc::new(Self {
            runtime,
            catalog,
            base,
            max_depth: MAX_DEPTH,
            model,
            allow_background: false,
        })
    }

    /// The parent's tools plus one tool per child agent.
    #[must_use]
    pub fn registry(self: &Arc<Self>) -> Registry {
        if self.catalog.is_empty() {
            return self.base.clone();
        }
        self.base.extend(self.tools())
    }

    /// Validate every definition against the process tool catalog.
    ///
    /// # Errors
    ///
    /// Returns a stable definition error before the service accepts work.
    pub fn validate(self: &Arc<Self>) -> Result<(), crate::Error> {
        let available = self.registry();
        for definition in self.catalog.iter() {
            let selected = available.select(definition.tools(), definition.disallowed_tools());
            crate::definition_validate::validate(definition, &self.catalog, &available, &selected)?;
        }
        Ok(())
    }

    fn tools(self: &Arc<Self>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(crate::spawn_tool::AgentTool(Arc::clone(self))),
            Arc::new(crate::message_tool::SendMessageTool(Arc::clone(self))),
            Arc::new(crate::output_tool::TaskOutputTool(Arc::clone(self))),
            Arc::new(crate::stop_tool::TaskStopTool(Arc::clone(self))),
        ]
    }
}
