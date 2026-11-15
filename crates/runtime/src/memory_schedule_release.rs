use crate::memory_state::State;
use crate::schedule_repeat::now_unix_ms;
use crate::{
    Address, Destination, LaneId, Operation, OperationId, OperationState, Prompt, ScheduleId,
    SessionId,
};

impl State {
    pub(crate) fn release_due(&mut self) {
        let due = self
            .schedules
            .due(now_unix_ms(), &self.operations, &self.unfollowed_lanes);
        let Some(id) = due else {
            return;
        };
        let row = self.schedules.rows[&id].clone();
        self.next_id += 1;
        let operation_id = OperationId(self.next_id);
        let mut prompt = run_prompt(id, &row);
        let (session_id, lane_id) = self.run_lane(&row, &mut prompt);
        let operation = Operation {
            id: operation_id,
            session_id,
            lane_id,
            state: OperationState::Accepted,
            outcome: None,
        };
        self.operations.insert(operation_id, operation);
        self.runs.register(operation_id, prompt.clone());
        self.deliveries.register(operation_id, prompt.delivery);
        self.schedules.runs.insert(operation_id, id);
        self.schedules.release(id, operation_id);
    }

    /// The origin's lane, or a new lane in the destination for a post.
    fn run_lane(
        &mut self,
        row: &crate::memory_schedule_row::Row,
        prompt: &mut Prompt,
    ) -> (SessionId, LaneId) {
        match &row.request.destination {
            None => (self.operations[&row.request.operation_id].session_id, row.lane_id),
            Some(destination) => self.open_lane(prompt, destination),
        }
    }

    /// A post's run gets a lane of its own in the destination, named by its
    /// request id until the surface binds it to the root it posts.
    fn open_lane(
        &mut self,
        prompt: &mut Prompt,
        destination: &Destination,
    ) -> (SessionId, LaneId) {
        prompt.address = Address::new(
            &destination.surface,
            &destination.conversation,
            &prompt.request_id,
        )
        .expect("a validated destination has a surface and conversation");
        prompt.identity = Some(destination.identity.clone());
        prompt.delivery = Some(destination.target.clone());
        self.addresses
            .resolve(prompt)
            .expect("a new lane accepts the destination's identity")
    }
}

/// The origin's prompt, carrying the schedule's words under the run's request id.
fn run_prompt(id: ScheduleId, row: &crate::memory_schedule_row::Row) -> Prompt {
    let mut prompt = row.prompt.clone();
    prompt.request_id = request_id(id, row.repeats(), row.scheduled.run_at_unix_ms);
    prompt.content.text.clone_from(&row.request.prompt);
    prompt.source_text.clone_from(&row.request.prompt);
    prompt
}

/// A standing schedule's runs share a lane, so each carries its run time.
fn request_id(id: ScheduleId, repeats: bool, run_at_unix_ms: i64) -> String {
    if repeats {
        return format!("schedule/{}/{run_at_unix_ms}", id.0);
    }
    format!("schedule/{}", id.0)
}
