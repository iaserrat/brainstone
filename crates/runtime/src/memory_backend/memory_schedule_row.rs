use crate::{LaneId, OperationId, Prompt, ScheduleRequest, ScheduleTime, Scheduled};

#[derive(Debug, Clone)]
pub(crate) struct Row {
    pub(crate) request: ScheduleRequest,
    pub(crate) scheduled: Scheduled,
    pub(crate) lane_id: LaneId,
    pub(crate) prompt: Prompt,
    /// The latest operation this row was promoted into.
    pub(crate) released: Option<OperationId>,
    pub(crate) consecutive_failures: i16,
    /// When the row was created; intervals and every-N rules count from it.
    pub(crate) anchor_unix_ms: i64,
}

impl Row {
    pub(crate) fn repeats(&self) -> bool {
        matches!(self.request.when, ScheduleTime::Repeat(_))
    }

    /// A schedule that replies in a thread no longer followed waits for a mention.
    pub(crate) fn paused(&self, unfollowed: &std::collections::HashSet<LaneId>) -> bool {
        self.request.destination.is_none() && unfollowed.contains(&self.lane_id)
    }
}

/// What a new row is made of besides its request.
pub(crate) struct Origin<'a> {
    pub(crate) lane_id: LaneId,
    pub(crate) prompt: &'a Prompt,
    pub(crate) anchor_unix_ms: i64,
}

impl Row {
    pub(crate) fn new(
        request: &ScheduleRequest,
        scheduled: Scheduled,
        origin: &Origin<'_>,
    ) -> Self {
        Self {
            request: request.clone(),
            scheduled,
            lane_id: origin.lane_id,
            prompt: origin.prompt.clone(),
            released: None,
            consecutive_failures: 0,
            anchor_unix_ms: origin.anchor_unix_ms,
        }
    }
}
