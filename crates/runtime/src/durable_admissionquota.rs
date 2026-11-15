use crate::{AdmissionQuota, DurableStore, Error};
use crate::durable::dispatch;

impl AdmissionQuota for DurableStore {
    async fn actor_operations_last_day(&self, actor: &str) -> Result<usize, Error> {
        dispatch!(self, actor_operations_last_day(actor))
    }

}
