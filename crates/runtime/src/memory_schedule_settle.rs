use agent::Outcome;

use crate::memory_schedule::Schedules;
use crate::memory_schedule_row::Row;
use crate::schedule_failure::REPEAT_FAILURE_CANCEL_AFTER;
use crate::{OperationId, ScheduleState};

#[cfg(test)]
#[path = "memory_schedule_repeat_tests.rs"]
mod tests;

impl Schedules {
    /// A finished run settles its standing schedule's failure streak: a success
    /// resets it, and a streak reaching the cancel threshold ends the schedule.
    pub(crate) fn settle(
        &mut self,
        operation: OperationId,
        outcome: &Outcome,
    ) -> Result<(), crate::Error> {
        let Some(row) = self.rows.values_mut().find(|row| {
            row.released == Some(operation)
                && row.repeats()
                && row.scheduled.state == ScheduleState::Pending
        }) else {
            return Ok(());
        };
        match outcome {
            Outcome::Completed { .. } => row.consecutive_failures = 0,
            Outcome::Failed { .. } => fail(row),
            _ => return Err(crate::Error::corrupt("unknown outcome")),
        }
        Ok(())
    }

    /// The failure streak behind a delivered operation, for its failure notice.
    pub(crate) fn consecutive_failures(&self, operation: OperationId) -> Option<i16> {
        self.rows
            .values()
            .find(|row| row.released == Some(operation))
            .map(|row| row.consecutive_failures)
    }
}

fn fail(row: &mut Row) {
    row.consecutive_failures += 1;
    if row.consecutive_failures >= REPEAT_FAILURE_CANCEL_AFTER {
        row.scheduled.state = ScheduleState::Cancelled;
    }
}
