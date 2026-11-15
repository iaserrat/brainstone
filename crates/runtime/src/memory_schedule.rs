use crate::memory_schedule_row::{Origin, Row};
use crate::memory_state::State;
use crate::schedule_destination::Creator;
use crate::schedule_repeat::now_unix_ms;
use crate::{
    Error, OperationId, Prompt, ScheduleId, ScheduleRequest, ScheduleState, ScheduleTime,
    Scheduled,
};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct Schedules {
    pub(crate) next_id: i64,
    pub(crate) rows: HashMap<ScheduleId, Row>,
    pub(crate) keys: HashMap<(OperationId, String), ScheduleId>,
    /// Which schedule released each run.
    pub(crate) runs: HashMap<OperationId, ScheduleId>,
}

impl State {
    pub(crate) fn schedule(&mut self, request: &ScheduleRequest) -> Result<Scheduled, Error> {
        let key = (request.operation_id, request.key.clone());
        if let Some(existing) = self.schedules.existing(&key, request)? {
            return Ok(existing);
        }
        let row = self.schedule_row(request)?;
        self.schedules.admit(&row)?;
        let scheduled = row.scheduled.clone();
        self.schedules.next_id += 1;
        self.schedules.keys.insert(key, scheduled.id);
        self.schedules.rows.insert(scheduled.id, row);
        Ok(scheduled)
    }

    fn schedule_row(&self, request: &ScheduleRequest) -> Result<Row, Error> {
        let operation = self
            .operations
            .get(&request.operation_id)
            .ok_or(Error::ScheduleNotFound)?;
        let origin = self
            .runs
            .prompt(request.operation_id)
            .ok_or(Error::ScheduleNotFound)?;
        if let Some(destination) = &request.destination {
            let creator = Creator {
                identity: origin.identity.as_ref(),
                has_target: origin.delivery.is_some(),
                scheduled_run: self.schedules.runs.contains_key(&request.operation_id),
            };
            crate::schedule_destination::check(&creator, destination)?;
        }
        let anchor_unix_ms = now_unix_ms();
        let scheduled = self.scheduled(request, origin, anchor_unix_ms)?;
        let origin = Origin {
            lane_id: operation.lane_id,
            prompt: origin,
            anchor_unix_ms,
        };
        Ok(Row::new(request, scheduled, &origin))
    }

    fn scheduled(
        &self,
        request: &ScheduleRequest,
        origin: &Prompt,
        now: i64,
    ) -> Result<Scheduled, Error> {
        let repeat = match &request.when {
            ScheduleTime::Repeat(recurrence) => Some(recurrence.clone()),
            _ => None,
        };
        Ok(Scheduled {
            id: ScheduleId(self.schedules.next_id + 1),
            prompt: request.prompt.clone(),
            run_at_unix_ms: crate::memory_schedule_time::run_at(&request.when, now)?,
            state: ScheduleState::Pending,
            repeat,
            owner: origin.identity.as_ref().map(|identity| identity.actor.clone()),
            destination: request
                .destination
                .as_ref()
                .map(|destination| destination.identity.conversation.clone()),
            paused: false,
        })
    }
}
