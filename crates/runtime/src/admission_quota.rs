use crate::Error;

/// How much work a surface actor has recently had accepted, so an admission
/// gate can enforce a per-actor budget before any durable write or model
/// spend.
pub trait AdmissionQuota: Clone + Send + Sync {
    /// Operations accepted for `actor` in the trailing 24 hours, by store
    /// time.
    fn actor_operations_last_day(
        &self,
        actor: &str,
    ) -> impl Future<Output = Result<usize, Error>> + Send;
}
