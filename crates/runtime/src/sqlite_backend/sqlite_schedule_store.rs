use crate::sqlite_backend::{sqlite_schedule, sqlite_schedule_manage};
use crate::{
    Error, OperationId, ScheduleId, ScheduleRequest, ScheduleStore, Scheduled, SqliteStore,
};

impl ScheduleStore for SqliteStore {
    async fn schedule(&self, request: &ScheduleRequest) -> Result<Scheduled, Error> {
        sqlite_schedule::create(&self.pool, request).await
    }

    async fn schedules(&self, operation_id: OperationId) -> Result<Vec<Scheduled>, Error> {
        sqlite_schedule_manage::list(&self.pool, operation_id).await
    }

    async fn cancel_schedule(
        &self,
        operation_id: OperationId,
        schedule_id: ScheduleId,
    ) -> Result<Scheduled, Error> {
        sqlite_schedule_manage::cancel(&self.pool, operation_id, schedule_id).await
    }
}
