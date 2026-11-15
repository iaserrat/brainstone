use std::fmt;

use super::Error;

impl fmt::Display for Error {
    #[allow(clippy::too_many_lines, reason = "flat dispatch: one arm per variant")]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { field } => write!(formatter, "invalid {field}"),
            Self::ExecutionIdentityConflict => {
                formatter.write_str("lane execution identity cannot change")
            }
            Self::IdempotencyConflict => formatter.write_str("request id reused with new input"),
            Self::LaneConfigurationConflict => {
                formatter.write_str("lane system prompt cannot change")
            }
            Self::DurabilityNotGuaranteed { setting, value } => {
                write!(formatter, "{setting} is {value}, expected on")
            }
            Self::CorruptRecord { message }
            | Self::StoreUnavailable { message }
            | Self::StoreFailed { message } => formatter.write_str(message),
            Self::SessionBusy => {
                formatter.write_str("session has an active execution or delivery lease")
            }
            Self::LeaseLost => formatter.write_str("operation lease was lost"),
            Self::InvalidCompaction => formatter.write_str("compaction boundary is invalid"),
            Self::ScheduleNotFound => formatter.write_str("scheduled work was not found"),
            Self::ScheduleLimitExceeded => {
                formatter.write_str("the lane already holds the maximum standing schedules")
            }
            Self::ScheduleOwnerLimitExceeded => {
                formatter.write_str("the owner already holds the maximum standing schedules")
            }
            Self::ScheduleDestinationForbidden => formatter.write_str(
                "only a person's own surface message can schedule a post, as them, into another conversation",
            ),
            Self::SchedulePostRateExceeded => {
                formatter.write_str("the owner created the maximum posts for this hour")
            }
            Self::InvalidDeliveryCheckpoint => {
                formatter.write_str("delivery checkpoint is invalid")
            }
            Self::AgentConcurrencyExceeded => {
                formatter.write_str("subagent concurrency limit reached")
            }
        }
    }
}
