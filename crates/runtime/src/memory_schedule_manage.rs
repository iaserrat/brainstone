use crate::memory_schedule_row::Row;
use crate::memory_state::State;
use crate::schedule_destination::in_destination;
use crate::schedule_owner::owner_in_private_conversation;
use crate::{Error, OperationId, ScheduleId, ScheduleState, Scheduled};

#[cfg(test)]
#[path = "memory_schedule_owner_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "memory_schedule_post_tests.rs"]
mod post_tests;

impl State {
    pub(crate) fn schedules(&self, operation_id: OperationId) -> Result<Vec<Scheduled>, Error> {
        let mut values = Vec::new();
        for row in self.schedules.rows.values() {
            if row.scheduled.state == ScheduleState::Pending && self.visible(operation_id, row)? {
                let mut scheduled = row.scheduled.clone();
                scheduled.paused = row.paused(&self.unfollowed_lanes);
                values.push(scheduled);
            }
        }
        values.sort_by_key(|item| (item.run_at_unix_ms, item.id.0));
        Ok(values)
    }

    pub(crate) fn cancel_schedule(
        &mut self,
        operation_id: OperationId,
        id: ScheduleId,
    ) -> Result<Scheduled, Error> {
        let row = self.schedules.rows.get(&id).ok_or(Error::ScheduleNotFound)?;
        if !self.visible(operation_id, row)? || row.scheduled.state == ScheduleState::Released {
            return Err(Error::ScheduleNotFound);
        }
        let row = self.schedules.rows.get_mut(&id).expect("row read above");
        row.scheduled.state = ScheduleState::Cancelled;
        Ok(row.scheduled.clone())
    }

    /// A schedule is visible in the lane that created it, to its owner from a
    /// private conversation, and to anyone in the conversation it posts into.
    fn visible(&self, caller: OperationId, row: &Row) -> Result<bool, Error> {
        let lane = self
            .operations
            .get(&caller)
            .ok_or(Error::ScheduleNotFound)?
            .lane_id;
        let identity = self.runs.prompt(caller).and_then(|prompt| prompt.identity.as_ref());
        Ok(row.lane_id == lane
            || owner_in_private_conversation(identity, row.prompt.identity.as_ref())
            || in_destination(identity, row.request.destination.as_ref()))
    }
}
