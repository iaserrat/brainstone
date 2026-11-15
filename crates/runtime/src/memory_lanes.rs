use crate::{Address, Error, LaneStore, MemoryStore, OperationId};

#[expect(clippy::unused_async_trait_impl, reason = "Preserve execution on poll across the async trait seam.")]
impl LaneStore for MemoryStore {
    async fn lane_is_followed(&self, address: &Address) -> Result<bool, Error> {
        let state = self.state.lock().expect("memory store poisoned");
        Ok(state
            .addresses
            .find_lane(address)
            .is_some_and(|id| !state.unfollowed_lanes.contains(&id)))
    }

    async fn follow_lane(&self, address: &Address, request_id: &str) -> Result<(), Error> {
        let mut state = self.state.lock().expect("memory store poisoned");
        let Some(id) = state.addresses.find_lane(address) else {
            return Ok(());
        };
        if state.follow_activations.insert((id, request_id.to_owned())) {
            state.unfollowed_lanes.remove(&id);
        }
        Ok(())
    }

    async fn unfollow_lane(&self, operation_id: OperationId, call_key: &str) -> Result<(), Error> {
        let mut state = self.state.lock().expect("memory store poisoned");
        let lane_id = state
            .operations
            .get(&operation_id)
            .ok_or(Error::InvalidInput {
                field: "operation_id",
            })?
            .lane_id;
        if let Some(existing) = state.unfollow_calls.get(call_key) {
            return (*existing == lane_id)
                .then_some(())
                .ok_or(Error::IdempotencyConflict);
        }
        state.unfollow_calls.insert(call_key.to_owned(), lane_id);
        state.unfollowed_lanes.insert(lane_id);
        Ok(())
    }

    async fn request_exists(&self, address: &Address, request_id: &str) -> Result<bool, Error> {
        Ok(self
            .state
            .lock()
            .expect("memory store poisoned")
            .request_exists(address, request_id))
    }
}
