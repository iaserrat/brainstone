use std::collections::BTreeMap;

use crate::{Diagnostic, Skill};

/// The skills found under an agent's `skills/` directory, with load diagnostics.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub(crate) skills: BTreeMap<String, Skill>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl Catalog {
    #[must_use]
    /// The skill loaded under `name`, if any.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    #[must_use]
    /// Every problem met while loading, in discovery order.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Every loaded skill, ordered by name.
    pub fn iter(&self) -> impl Iterator<Item = &Skill> {
        self.skills.values()
    }

    #[must_use]
    /// Whether no skill loaded.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}
