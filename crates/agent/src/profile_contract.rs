use super::ExecutionProfile;
use crate::ToolContract;

impl ExecutionProfile {
    /// Pin capability, approval, and completion requirements in accepted records.
    #[must_use]
    pub fn with_tool_contract(mut self, contract: ToolContract) -> Self {
        self.tool_contract = Some(contract);
        self
    }

    /// The accepted tool contract, or `None` for legacy unrestricted behavior.
    #[must_use]
    pub const fn tool_contract(&self) -> Option<&ToolContract> {
        self.tool_contract.as_ref()
    }
}
