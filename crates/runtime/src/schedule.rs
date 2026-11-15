use serde::{Deserialize, Serialize};

use crate::{Destination, Error, OperationId, Recurrence};

/// When a schedule first runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleTime {
    /// At this instant, in Unix milliseconds.
    AtUnixMillisecond(i64),
    /// After this delay.
    AfterSeconds(u64),
    /// A standing schedule the runtime re-releases itself after every run.
    Repeat(Recurrence),
}

/// One schedule a caller asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleRequest {
    /// The operation that asked.
    pub operation_id: OperationId,
    /// The idempotency key for this request.
    pub key: String,
    /// The text each run receives.
    pub prompt: String,
    /// When the first run happens.
    pub when: ScheduleTime,
    /// `None` replies in the origin lane; `Some` posts every run elsewhere.
    pub destination: Option<Destination>,
}

impl ScheduleRequest {
    /// # Errors
    /// Returns `invalid_input` for empty identity or work.
    pub fn new(
        operation_id: OperationId,
        key: &str,
        prompt: &str,
        when: ScheduleTime,
    ) -> Result<Self, Error> {
        crate::prompt::require(key, "schedule_key")?;
        crate::prompt::require(prompt, "schedule_prompt")?;
        Ok(Self {
            operation_id,
            key: key.to_owned(),
            prompt: prompt.to_owned(),
            when,
            destination: None,
        })
    }

    #[must_use]
    /// Posts each run in `destination` instead of replying in place.
    pub fn with_destination(mut self, destination: Destination) -> Self {
        self.destination = Some(destination);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
/// The store's id for a schedule.
pub struct ScheduleId(pub(crate) i64);

impl ScheduleId {
    /// # Errors
    /// Returns `invalid_input` unless the identifier is positive.
    pub fn new(value: i64) -> Result<Self, Error> {
        (value > 0)
            .then_some(Self(value))
            .ok_or(Error::InvalidInput {
                field: "schedule_id",
            })
    }

    #[must_use]
    /// The raw id.
    pub const fn get(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Where a schedule stands.
#[non_exhaustive]
pub enum ScheduleState {
    /// Waiting for its next run.
    Pending,
    /// Stopped before its next run.
    Cancelled,
    /// No longer owned by the lane that asked.
    Released,
}

/// A stored schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scheduled {
    /// Its id.
    pub id: ScheduleId,
    /// The text each run receives.
    pub prompt: String,
    /// The next run for a standing schedule; the only run otherwise.
    pub run_at_unix_ms: i64,
    /// Where it stands.
    pub state: ScheduleState,
    /// Its recurrence, or `None` for a single run.
    pub repeat: Option<Recurrence>,
    /// The actor who created it, when that operation was authenticated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The conversation each run posts a new message in; absent when runs reply in place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    /// The surface stopped following the destination conversation: no run is
    /// released until it follows again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub paused: bool,
}
