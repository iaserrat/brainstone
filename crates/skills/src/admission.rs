use host::Admission;
use runtime::{OperationId, Prompt};

use crate::Catalog;

/// Wraps `inner`, adding the skills index to each accepted prompt.
#[derive(Clone, Debug)]
pub struct SkillAdmission<A> {
    inner: A,
    catalog: Catalog,
}

impl<A> SkillAdmission<A> {
    #[must_use]
    /// Wraps `inner` with `catalog`'s index.
    pub const fn new(inner: A, catalog: Catalog) -> Self {
        Self { inner, catalog }
    }
}

impl<A: Admission> Admission for SkillAdmission<A> {
    async fn preflight(&self, actor: Option<&str>) -> Result<(), host::Error> {
        self.inner.preflight(actor).await
    }

    async fn accept(&self, prompt: &Prompt) -> Result<OperationId, host::Error> {
        let text = self.catalog.expand_mentions(prompt.text());
        let expanded = prompt.clone().with_text(text)?;
        self.inner.accept(&expanded).await
    }
}
