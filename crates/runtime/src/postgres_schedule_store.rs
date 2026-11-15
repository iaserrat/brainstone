use crate::{
    Error, OperationId, PostgresStore, ScheduleId, ScheduleRequest, ScheduleStore, Scheduled,
};

impl ScheduleStore for PostgresStore {
    async fn schedule(&self, request: &ScheduleRequest) -> Result<Scheduled, Error> {
        crate::postgres_schedule::create(&self.pool, request).await
    }

    async fn schedules(&self, operation_id: OperationId) -> Result<Vec<Scheduled>, Error> {
        crate::postgres_schedule_manage::list(&self.pool, operation_id).await
    }

    async fn cancel_schedule(
        &self,
        operation_id: OperationId,
        schedule_id: ScheduleId,
    ) -> Result<Scheduled, Error> {
        crate::postgres_schedule_manage::cancel(&self.pool, operation_id, schedule_id).await
    }
}
