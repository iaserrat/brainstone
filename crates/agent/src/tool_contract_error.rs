/// A malformed accepted tool contract.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToolContractError {
    /// Names must contain 1–64 ASCII letters, digits, underscores, or hyphens.
    InvalidName,
    /// An approval or success requirement names a tool the contract forbids.
    ToolNotAllowed,
}

/// The accepted tool contract is malformed or internally inconsistent.
pub const CODE_INVALID_TOOL_CONTRACT: &str = "invalid_tool_contract";

impl ToolContractError {
    /// Stable machine-readable failure code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        CODE_INVALID_TOOL_CONTRACT
    }
}
impl std::fmt::Display for ToolContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidName => "invalid exact tool name",
            Self::ToolNotAllowed => "required tool is excluded by the accepted allowlist",
        })
    }
}
impl std::error::Error for ToolContractError {}
