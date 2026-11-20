use std::collections::{HashMap, HashSet};

use crate::memory_backend::memory_schedule::Schedules;
use crate::schedule_repeat::now_unix_ms;
use crate::{LaneId, Operation, OperationId, OperationState, ScheduleId, ScheduleState};

#[cfg(test)]
#[path = "memory_schedule_stop_tests.rs"]
mod tests;

impl Schedules {
    /// The earliest due row whose previous run, if any, has ended: a standing
    /// schedule never overlaps itself. A paused row is never due.
    pub(crate) fn due(
        &self,
        now: i64,
        operations: &HashMap<OperationId, Operation>,
        unfollowed: &HashSet<LaneId>,
    ) -> Option<ScheduleId> {
        self.rows
            .values()
            .filter(|row| {
                row.scheduled.state == ScheduleState::Pending
                    && row.scheduled.run_at_unix_ms <= now
                    && row.released.is_none_or(|id| terminal(operations, id))
                    && !row.paused(unfollowed)
            })
            .min_by_key(|row| (row.scheduled.run_at_unix_ms, row.scheduled.id.0))
            .map(|row| row.scheduled.id)
    }

    pub(crate) fn release(&mut self, id: ScheduleId, operation: OperationId) {
        let row = self.rows.get_mut(&id).expect("due schedule exists");
        row.released = Some(operation);
        match &row.request.when {
            crate::ScheduleTime::Repeat(recurrence) => {
                row.scheduled.run_at_unix_ms = recurrence
                    .next_run_at(now_unix_ms(), row.anchor_unix_ms)
                    .expect("a validated recurrence always has a next occurrence");
            }
            _ => row.scheduled.state = ScheduleState::Released,
        }
    }
}

fn terminal(operations: &HashMap<OperationId, Operation>, id: OperationId) -> bool {
    operations.get(&id).is_none_or(|operation| {
        matches!(
            operation.state,
            OperationState::Completed | OperationState::Failed
        )
    })
}
