use std::sync::Arc;

use agent::{Registry, Tool};

use crate::Catalog;

impl Catalog {
    #[must_use]
    /// Adds the skill tools to `base`, or returns it unchanged when nothing loaded.
    pub fn registry(&self, base: Registry) -> Registry {
        if self.is_empty() {
            return base;
        }
        let catalog = Arc::new(self.clone());
        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(crate::skill_tool::SkillTool(Arc::clone(&catalog))),
            Arc::new(crate::resource_tool::ResourceTool(catalog)),
        ];
        base.extend(tools)
    }
}
