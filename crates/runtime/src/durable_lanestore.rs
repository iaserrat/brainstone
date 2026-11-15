use crate::{Address, DurableStore, Error, LaneStore};
use crate::durable::dispatch;

impl LaneStore for DurableStore {
    async fn lane_is_followed(&self, address: &Address) -> Result<bool, Error> {
        dispatch!(self, lane_is_followed(address))
    }

    async fn follow_lane(&self, address: &Address, request_id: &str) -> Result<(), Error> {
        dispatch!(self, follow_lane(address, request_id))
    }

    async fn unfollow_lane(
        &self,
        operation_id: crate::OperationId,
        call_key: &str,
    ) -> Result<(), Error> {
        dispatch!(self, unfollow_lane(operation_id, call_key))
    }

    async fn request_exists(&self, address: &Address, request_id: &str) -> Result<bool, Error> {
        dispatch!(self, request_exists(address, request_id))
    }

}
