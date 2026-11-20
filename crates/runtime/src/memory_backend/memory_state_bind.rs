use crate::memory_backend::memory_state::State;
use crate::{DeliveryLease, DeliveryTarget, Error};

impl State {
    /// A post's lane takes the root's name, and its delivery the reply route,
    /// under the delivery lease that posted the root.
    pub(crate) fn bind_lane(
        &mut self,
        lease: &DeliveryLease,
        external_id: &str,
        target: &DeliveryTarget,
    ) -> Result<bool, Error> {
        let id = lease.operation_id;
        self.deliveries
            .rows
            .get(&id)
            .ok_or(Error::LeaseLost)?
            .validate(lease)?;
        if !self.schedules.runs.contains_key(&id) {
            return Err(Error::InvalidInput { field: "lane" });
        }
        let lane = self.operations.get(&id).ok_or(Error::LeaseLost)?.lane_id;
        let provisional = self
            .runs
            .prompt(id)
            .ok_or(Error::LeaseLost)?
            .request_id
            .clone();
        let bound = self.addresses.rename(lane, &provisional, external_id)?;
        if bound {
            let row = self.deliveries.rows.get_mut(&id).expect("validated above");
            row.target = target.clone();
        }
        Ok(bound)
    }
}
