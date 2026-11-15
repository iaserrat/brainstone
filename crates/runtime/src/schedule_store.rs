use crate::{Error, OperationId, ScheduleId, ScheduleRequest, Scheduled};

/// Durable schedules: create, list, and cancel.
pub trait ScheduleStore: Clone + Send + Sync {
    /// Stores a schedule, or returns the one the same key already stored.
    fn schedule(
        &self,
        request: &ScheduleRequest,
    ) -> impl Future<Output = Result<Scheduled, Error>> + Send;

    /// The schedules the asking lane owns, oldest first.
    fn schedules(
        &self,
        operation_id: OperationId,
    ) -> impl Future<Output = Result<Vec<Scheduled>, Error>> + Send;

    /// Stops a schedule the caller owns.
    fn cancel_schedule(
        &self,
        operation_id: OperationId,
        schedule_id: ScheduleId,
    ) -> impl Future<Output = Result<Scheduled, Error>> + Send;
}
