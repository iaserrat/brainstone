use std::error;
use std::fmt;

/// Expected runtime failures. Each variant carries a stable code that callers,
/// surfaces, and recovery branch on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A caller-supplied value is outside its accepted range.
    InvalidInput {
        /// The rejected input, by name.
        field: &'static str,
    },
    /// A retry reused a key with different content.
    IdempotencyConflict,
    /// A retry carried a different authenticated identity.
    ExecutionIdentityConflict,
    /// A retry carried a different system prompt or profile.
    LaneConfigurationConflict,
    /// The backend cannot guarantee durability with this setting.
    DurabilityNotGuaranteed {
        /// The server setting.
        setting: &'static str,
        /// Its current value.
        value: String,
    },
    /// A stored record cannot be interpreted; recovery refuses it.
    CorruptRecord {
        /// What was unreadable.
        message: String,
    },
    /// The lease expired or was reassigned; the worker must stop.
    LeaseLost,
    /// Another process holds this session's write claim.
    SessionBusy,
    /// The compaction would replace nothing, or everything.
    InvalidCompaction,
    /// No schedule with that id belongs to the caller.
    ScheduleNotFound,
    /// The lane reached its standing schedule limit.
    ScheduleLimitExceeded,
    /// The owner reached its standing schedule limit.
    ScheduleOwnerLimitExceeded,
    /// Policy refuses posting in that conversation.
    ScheduleDestinationForbidden,
    /// The owner posted more often than its allowed rate.
    SchedulePostRateExceeded,
    /// The checkpoint does not follow the previous one.
    InvalidDeliveryCheckpoint,
    /// The store cannot be reached right now.
    StoreUnavailable {
        /// The transport's message.
        message: String,
    },
    /// The store rejected or failed the statement.
    StoreFailed {
        /// The database's message.
        message: String,
    },
    /// The session runs as many child agents as it may.
    AgentConcurrencyExceeded,
}

impl Error {
    #[must_use]
    /// The stable code a caller branches on.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "invalid_input",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::ExecutionIdentityConflict => "execution_identity_conflict",
            Self::LaneConfigurationConflict => "lane_configuration_conflict",
            Self::DurabilityNotGuaranteed { .. } => "durability_not_guaranteed",
            Self::CorruptRecord { .. } => "corrupt_record",
            Self::LeaseLost => "lease_lost",
            Self::SessionBusy => "session_busy",
            Self::InvalidCompaction => "invalid_compaction",
            Self::ScheduleNotFound => "schedule_not_found",
            Self::ScheduleLimitExceeded => "schedule_limit_exceeded",
            Self::ScheduleOwnerLimitExceeded => "schedule_owner_limit_exceeded",
            Self::ScheduleDestinationForbidden => "schedule_destination_forbidden",
            Self::SchedulePostRateExceeded => "schedule_post_rate_exceeded",
            Self::InvalidDeliveryCheckpoint => "invalid_delivery_checkpoint",
            Self::StoreUnavailable { .. } => "store_unavailable",
            Self::StoreFailed { .. } => "store_failed",
            Self::AgentConcurrencyExceeded => "agent_concurrency_exceeded",
        }
    }
}

#[path = "error_display.rs"]
mod display;

impl error::Error for Error {}

impl Error {
    pub(crate) fn corrupt(error: impl fmt::Display) -> Self {
        Self::CorruptRecord {
            message: error.to_string(),
        }
    }
}
