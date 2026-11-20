use agent::ExecutionIdentity;

use crate::memory_backend::memory_schedule::Schedules;
use crate::memory_backend::memory_schedule_row::Row;
use crate::schedule_owner::{
    HOUR_MS, MAX_POSTS_PER_OWNER_PER_HOUR, MAX_REPEAT_PER_OWNER, same_owner,
};
use crate::schedule_repeat::MAX_REPEAT_PER_LANE;
use crate::{Error, OperationId, ScheduleRequest, ScheduleState, Scheduled};

impl Schedules {
    /// A new standing row fits under its lane's and its owner's caps, and a
    /// new post under its owner's hourly rate.
    pub(crate) fn admit(&self, row: &Row) -> Result<(), Error> {
        let in_lane = |other: &Row| other.lane_id == row.lane_id;
        if row.repeats() && self.standing(in_lane) >= MAX_REPEAT_PER_LANE {
            return Err(Error::ScheduleLimitExceeded);
        }
        match row.prompt.identity.as_ref() {
            Some(owner) => self.admit_owned(row, owner),
            None => Ok(()),
        }
    }

    fn admit_owned(&self, row: &Row, owner: &ExecutionIdentity) -> Result<(), Error> {
        let same_person = |other: &Row| {
            other
                .prompt
                .identity
                .as_ref()
                .is_some_and(|identity| same_owner(identity, owner))
        };
        if row.repeats() && self.standing(same_person) >= MAX_REPEAT_PER_OWNER {
            return Err(Error::ScheduleOwnerLimitExceeded);
        }
        let recent_post = |other: &Row| {
            other.request.destination.is_some()
                && other.anchor_unix_ms > row.anchor_unix_ms - HOUR_MS
                && same_person(other)
        };
        if row.request.destination.is_some()
            && self.count(recent_post) >= MAX_POSTS_PER_OWNER_PER_HOUR
        {
            return Err(Error::SchedulePostRateExceeded);
        }
        Ok(())
    }

    fn standing(&self, filter: impl Fn(&Row) -> bool) -> i64 {
        self.count(|row| {
            row.repeats() && row.scheduled.state == ScheduleState::Pending && filter(row)
        })
    }

    fn count(&self, filter: impl Fn(&Row) -> bool) -> i64 {
        let count = self.rows.values().filter(|row| filter(row)).count();
        i64::try_from(count).expect("schedule count fits i64")
    }
}

impl Schedules {
    /// The row an earlier call with the same key made; a changed request conflicts.
    pub(crate) fn existing(
        &self,
        key: &(OperationId, String),
        request: &ScheduleRequest,
    ) -> Result<Option<Scheduled>, Error> {
        let Some(id) = self.keys.get(key) else {
            return Ok(None);
        };
        let row = &self.rows[id];
        if row.request != *request {
            return Err(Error::IdempotencyConflict);
        }
        Ok(Some(row.scheduled.clone()))
    }
}
