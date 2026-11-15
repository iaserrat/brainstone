use std::time::Duration;

use crate::{CompactionPolicy, Error};

const MIN_LEASE_TTL: Duration = Duration::from_millis(3);

/// Runtime tunables: worker identity, lease timing, and policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub(crate) worker: String,
    pub(crate) lease_ttl: Duration,
    pub(crate) compaction: Option<CompactionPolicy>,
    pub(crate) tool_policy: Option<Box<crate::ToolPolicy>>,
}

impl Config {
    pub(crate) fn automatic(worker: String, lease_ttl: Duration) -> Self {
        Self {
            worker,
            lease_ttl,
            compaction: None,
            tool_policy: None,
        }
    }

    /// # Errors
    ///
    /// Returns `invalid_input` for an empty worker identity or too-short lease.
    pub fn new(worker: &str, lease_ttl: Duration) -> Result<Self, Error> {
        if worker.is_empty() {
            return Err(Error::InvalidInput { field: "worker" });
        }
        if lease_ttl < MIN_LEASE_TTL {
            return Err(Error::InvalidInput { field: "lease_ttl" });
        }
        Ok(Self {
            worker: worker.to_owned(),
            lease_ttl,
            compaction: None,
            tool_policy: None,
        })
    }

    #[must_use]
    /// Sets the automatic compaction policy.
    pub fn with_compaction(mut self, policy: CompactionPolicy) -> Self {
        self.compaction = Some(policy);
        self
    }
    #[must_use]
    /// Sets the operator's tool policy.
    pub fn with_tool_policy(mut self, policy: crate::ToolPolicy) -> Self {
        self.tool_policy = Some(Box::new(policy));
        self
    }
}
