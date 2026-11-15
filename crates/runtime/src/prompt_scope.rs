use crate::{MemoryScope, Prompt};
impl Prompt {
    #[must_use]
/// The memory scope this prompt's operation belongs to.
    pub fn memory_scope(&self) -> MemoryScope {
        match &self.identity {
            Some(identity) => MemoryScope::authenticated(identity),
            None => MemoryScope { surface: self.address.surface.clone(),
                conversation: self.address.conversation.clone(), actor: self.actor.clone() },
        }
    }
}
impl MemoryScope {
    #[must_use]
/// The scope an authenticated identity owns.
    pub fn authenticated(identity: &agent::ExecutionIdentity) -> Self {
        Self { surface: identity.scope(), conversation: identity.conversation_key(),
            actor: Some(identity.actor.clone()) }
    }
}
