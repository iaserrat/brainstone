use crate::Error;

/// Exact tool names (or `*`) selected by the operator, never by tool arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPolicy {
    pub(crate) require: Vec<String>,
    pub(crate) deny: Vec<String>,
    pub(crate) binding: String,
}
impl ToolPolicy {
    /// Current policy and executor binding. Deny wins over approval requirements.
    /// # Errors
    /// Rejects empty bindings and malformed tool names.
    pub fn new(require: Vec<String>, deny: Vec<String>, binding: String) -> Result<Self, Error> {
        if binding.is_empty() || require.iter().chain(&deny).any(|name| !valid(name)) {
            return Err(Error::InvalidInput {
                field: "tool_policy",
            });
        }
        Ok(Self {
            require,
            deny,
            binding,
        })
    }
    pub(crate) fn denied(&self, name: &str) -> bool {
        selected(&self.deny, name)
    }
    pub(crate) fn required(&self, name: &str) -> bool {
        selected(&self.require, name)
    }
}
fn selected(names: &[String], name: &str) -> bool {
    names.iter().any(|value| value == "*" || value == name)
}
fn valid(name: &str) -> bool {
    name == "*"
        || (!name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
}
