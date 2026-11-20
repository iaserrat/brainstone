use serde::{Deserialize, Serialize};

use super::ExecutionProfile;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum ToolKeyVersion {
    #[default]
    #[serde(rename = "legacy")]
    Legacy,
    #[serde(rename = "operation-v2")]
    Operation,
}

impl ToolKeyVersion {
    pub(super) const fn is_legacy(&self) -> bool {
        matches!(self, Self::Legacy)
    }
}

impl Default for ExecutionProfile {
    fn default() -> Self {
        Self::new(None, None, None, 0).expect("unlimited default profile is valid")
    }
}

impl ExecutionProfile {
    /// Snapshots predating profiles used legacy keys and a finite response limit.
    pub(crate) fn legacy() -> Self {
        Self {
            tool_key_version: ToolKeyVersion::Legacy,
            ..Self::new(None, None, crate::MAX_STEPS, 0).expect("legacy response limit is positive")
        }
    }

    pub(crate) const fn operation_tool_keys(&self) -> bool {
        matches!(self.tool_key_version, ToolKeyVersion::Operation)
    }

    /// Whether model settings match, ignoring the internal durable key version.
    /// Existing lanes retain their stored key scheme during strict admission.
    #[must_use]
    pub fn same_settings(&self, other: &Self) -> bool {
        self.model == other.model
            && self.effort == other.effort
            && self.max_turns == other.max_turns
            && self.depth == other.depth
            && self.allowed_agents == other.allowed_agents
            && self.output_schema == other.output_schema
            && self.tool_contract == other.tool_contract
    }
}
