//! Accepted capability and completion requirements, independent of tool handlers.
use serde::{Deserialize, Serialize};

#[path = "tool_contract_error.rs"]
mod error;
#[path = "tool_contract_validation.rs"]
mod validation;
pub use error::{CODE_INVALID_TOOL_CONTRACT, ToolContractError};

/// Exact tool names permitted, approval-gated, and required by an operation.
///
/// Names are sorted and deduplicated. Missing `allow` permits all tools; an
/// empty list permits none. Default permits all registered tools, without approval
/// or success requirements. Names contain 1..=64 ASCII letters, digits, underscores
/// or hyphens. Stored unknown fields and inconsistent sets reject.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawContract")]
pub struct ToolContract {
    allow: Option<Vec<String>>,
    require_approval: Vec<String>,
    require_success: Vec<String>,
}

impl ToolContract {
    /// Permit only these exact names. Empty permits none; `*` is not a name.
    /// # Errors
    /// Rejects malformed names with `invalid_tool_contract`.
    pub fn only(
        names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, ToolContractError> {
        Ok(Self {
            allow: Some(validation::names(names)?),
            ..Self::default()
        })
    }

    /// Replace approval requirements. Decisions use the runtime's durable approval flow.
    /// # Errors
    /// Rejects malformed names or names excluded by this contract.
    pub fn require_approval(
        mut self,
        names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, ToolContractError> {
        self.require_approval = validation::names(names)?;
        self.validate()?;
        Ok(self)
    }

    /// Replace completion requirements. Each name needs a successful committed
    /// tool result in this operation, not merely a model claim or earlier history.
    /// A successful handler return does not prove external business correctness.
    /// Missing results fail completion with `tool_requirement_unmet`, without a
    /// model repair call. Earlier failed outcomes remain failures.
    /// # Errors
    /// Rejects malformed names or names excluded by this contract.
    pub fn require_success(
        mut self,
        names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, ToolContractError> {
        self.require_success = validation::names(names)?;
        self.validate()?;
        Ok(self)
    }

    /// Whether the accepted contract permits this exact name.
    #[must_use]
    pub fn allows(&self, name: &str) -> bool {
        self.allow
            .as_ref()
            .is_none_or(|names| names.iter().any(|value| value == name))
    }

    /// Whether this exact name requires durable approval before dispatch.
    #[must_use]
    pub fn approval_required(&self, name: &str) -> bool {
        self.require_approval.iter().any(|value| value == name)
    }

    /// Sorted names each requiring at least one committed successful result.
    #[must_use]
    // software-design-allow: exposed-collection immutable sorted names are the public contract
    pub fn required_tools(&self) -> &[String] {
        &self.require_success
    }

    /// Sorted names each requiring approval before dispatch.
    #[must_use]
    // software-design-allow: exposed-collection immutable sorted names are the public contract
    pub fn approval_tools(&self) -> &[String] {
        &self.require_approval
    }

    /// The exact allowlist, or `None` when every registered tool is permitted.
    #[must_use]
    pub fn allowed_tools(&self) -> Option<&[String]> {
        self.allow.as_deref()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawContract {
    #[serde(default)]
    allow: Option<Vec<String>>,
    #[serde(default)]
    require_approval: Vec<String>,
    #[serde(default)]
    require_success: Vec<String>,
}

impl TryFrom<RawContract> for ToolContract {
    type Error = ToolContractError;
    fn try_from(raw: RawContract) -> Result<Self, Self::Error> {
        let contract = Self {
            allow: raw.allow.map(validation::names).transpose()?,
            require_approval: validation::names(raw.require_approval)?,
            require_success: validation::names(raw.require_success)?,
        };
        contract.validate()?;
        Ok(contract)
    }
}
