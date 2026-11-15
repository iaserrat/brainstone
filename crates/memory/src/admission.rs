use host::Admission;
use runtime::{Memories, OperationId, Prompt};

use crate::Scope;

/// Prepends recent memories to each prompt before durable acceptance, so the
/// block is part of the stored prompt and recovery replays it unchanged. The
/// user text is a tail append per operation, which keeps the provider cache
/// invariant that only compaction may rewrite earlier context.
#[derive(Clone, Debug)]
pub struct MemoryAdmission<A, S> {
    inner: A,
    store: S,
    scope: Scope,
    inject: Option<usize>,
}

impl<A, S> MemoryAdmission<A, S> {
    #[must_use]
    /// Wraps `inner`, injecting at most `inject` memories from `store` for `scope`.
    pub const fn new(inner: A, store: S, scope: Scope, inject: Option<usize>) -> Self {
        Self {
            inner,
            store,
            scope,
            inject,
        }
    }
}

impl<A: Admission, S: Memories> Admission for MemoryAdmission<A, S> {
    async fn preflight(&self, actor: Option<&str>) -> Result<(), host::Error> {
        self.inner.preflight(actor).await
    }

    async fn accept(&self, prompt: &Prompt) -> Result<OperationId, host::Error> {
        let Some(limit) = self
            .inject
            .filter(|_| self.scope.permits(prompt.identity()))
        else {
            return self.inner.accept(prompt).await;
        };
        let identity = prompt.memory_scope();
        // A scope needing an actor the prompt lacks skips injection rather
        // than rejecting the prompt: injection is an enhancement, and the
        // memory tools still fail with `memory_actor_required`.
        let Some(scope) = self.scope.key(
            &identity.surface,
            &identity.conversation,
            identity.actor.as_deref(),
        ) else {
            return self.inner.accept(prompt).await;
        };
        let memories = self.store.recall_memories(&scope, None, limit).await?;
        if memories.is_empty() {
            return self.inner.accept(prompt).await;
        }
        let injected = prompt.clone().with_text(block(&memories, prompt.text()))?;
        self.inner.accept(&injected).await
    }
}

/// Oldest first, so a later correction reads after the fact it corrects.
fn block(memories: &[String], text: &str) -> String {
    let mut lines = String::from("<memories>\n");
    for memory in memories.iter().rev() {
        lines.push_str("- ");
        lines.push_str(memory);
        lines.push('\n');
    }
    format!("{lines}</memories>\n\n{text}")
}
