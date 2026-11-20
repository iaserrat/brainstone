use super::{RequestKey, State};

pub(crate) fn request_key(prompt: &crate::Prompt) -> RequestKey {
    (
        prompt.address.surface.clone(),
        prompt.address.conversation.clone(),
        prompt.address.lane.clone(),
        prompt.request_id.clone(),
    )
}

impl State {
    pub(crate) fn request_exists(&self, address: &crate::Address, request_id: &str) -> bool {
        let key = (
            address.surface.clone(),
            address.conversation.clone(),
            address.lane.clone(),
            request_id.to_owned(),
        );
        self.requests.contains_key(&key) || self.steerings.rows.contains_key(&key)
    }
}
