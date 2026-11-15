use serde::{Deserialize, Serialize};

use crate::TurnLimit;

pub use crate::profile_error::ProfileError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// How much reasoning a turn may spend.
pub enum Effort {
    /// Little reasoning.
    Low,
    /// Moderate reasoning.
    Medium,
    /// More reasoning than `medium`.
    High,
    /// Extensive reasoning.
    XHigh,
    /// The most reasoning the model allows.
    Max,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// The settings one turn runs under, stored with the thread so recovery
/// replays the same model, effort, and limits.
pub struct ExecutionProfile {
    pub(crate) model: Option<String>,
    effort: Option<Effort>,
    max_turns: TurnLimit,
    depth: u8,
    #[serde(default)]
    allowed_agents: Option<Vec<String>>,
}

impl ExecutionProfile {
    /// Snapshots predating profiles used the original implicit response limit.
    pub(crate) fn legacy() -> Self {
        Self::new(None, None, crate::MAX_STEPS, 0).expect("legacy response limit is positive")
    }

    /// `None` permits unlimited responses; a positive count sets a limit.
    /// Existing integer arguments select that explicit limit.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an empty model or zero turn budget.
    pub fn new(
        model: Option<&str>,
        effort: Option<Effort>,
        max_turns: impl Into<Option<usize>>,
        depth: u8,
    ) -> Result<Self, ProfileError> {
        if model.is_some_and(str::is_empty) {
            return Err(ProfileError::EmptyModel);
        }
        Ok(Self {
            model: model.map(str::to_owned),
            effort,
            max_turns: TurnLimit::try_from(max_turns.into())?,
            depth,
            allowed_agents: None,
        })
    }

    #[must_use]
    /// The model the turn asks for, or the deployment's when `None`.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }
    #[must_use]
    /// The reasoning effort the turn asks for.
    pub const fn effort(&self) -> Option<Effort> {
        self.effort
    }
    /// The accepted response limit; `None` means unlimited.
    #[must_use]
    pub const fn max_turns(&self) -> Option<usize> {
        self.max_turns.get()
    }
    #[must_use]
    /// How deep the delegation chain is, so nesting stays bounded.
    pub const fn depth(&self) -> u8 {
        self.depth
    }
    #[must_use]
    /// The child agents this turn may delegate to, or `None` for any.
    pub fn allowed_agents(&self) -> Option<&[String]> {
        self.allowed_agents.as_deref()
    }
    /// The same profile at a different reasoning depth.
    #[must_use]
    pub fn with_effort(mut self, effort: Effort) -> Self {
        self.effort = Some(effort);
        self
    }
    #[must_use]
    /// Restricts delegation to `allowed`.
    pub fn with_allowed_agents(mut self, allowed: Option<Vec<String>>) -> Self {
        self.allowed_agents = allowed;
        self
    }
}
