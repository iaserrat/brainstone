use crate::{DurableStore, Error, OperationId, ScheduleId, ScheduleRequest, ScheduleStore, Scheduled};
use crate::durable::dispatch;

impl ScheduleStore for DurableStore {
    async fn schedule(&self, request: &ScheduleRequest) -> Result<Scheduled, Error> {
        dispatch!(self, schedule(request))
    }

    async fn schedules(&self, operation_id: OperationId) -> Result<Vec<Scheduled>, Error> {
        dispatch!(self, schedules(operation_id))
    }

    async fn cancel_schedule(
        &self,
        operation_id: OperationId,
        schedule_id: ScheduleId,
    ) -> Result<Scheduled, Error> {
        dispatch!(self, cancel_schedule(operation_id, schedule_id))
    }

}
