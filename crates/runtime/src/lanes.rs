use crate::{Address, Error, OperationId};

/// Durable lane-follow state used by surface ingress.
pub trait LaneStore: Clone + Send + Sync {
    /// Whether the lane is followed, so its events are delivered.
    fn lane_is_followed(
        &self,
        address: &Address,
    ) -> impl Future<Output = Result<bool, Error>> + Send;

    /// Marks the lane followed.
    fn follow_lane(
        &self,
        address: &Address,
        request_id: &str,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Marks the lane no longer followed.
    fn unfollow_lane(
        &self,
        operation_id: OperationId,
        call_key: &str,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Whether this request id was already accepted, for idempotent admission.
    fn request_exists(
        &self,
        _address: &Address,
        _request_id: &str,
    ) -> impl Future<Output = Result<bool, Error>> + Send {
        async { Ok(false) }
    }
}
