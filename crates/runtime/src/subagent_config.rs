use agent::ExecutionProfile;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// What a child run needs: its name, system prompt, profile, tools, and mode.
pub struct SubagentConfig {
    pub(crate) name: String,
    pub(crate) system: String,
    pub(crate) profile: ExecutionProfile,
    pub(crate) tools: Vec<Value>,
    pub(crate) background: bool,
}

impl SubagentConfig {
    /// # Errors
    /// Returns `invalid_input` when the name or system prompt is empty.
    pub fn new(name: &str, system: &str, profile: ExecutionProfile) -> Result<Self, Error> {
        if name.is_empty() {
            return Err(Error::InvalidInput {
                field: "agent_name",
            });
        }
        if system.trim().is_empty() {
            return Err(Error::InvalidInput {
                field: "agent_system",
            });
        }
        Ok(Self {
            name: name.to_owned(),
            system: system.to_owned(),
            profile,
            tools: Vec::new(),
            background: false,
        })
    }

    #[must_use]
    /// Offers `tools` to the child.
    pub fn with_tools(mut self, tools: Vec<Value>) -> Self {
        self.tools = tools;
        self
    }

    /// The names of the tools the child was spawned with: its grant for
    /// every later operation, resolved against the live registry.
    pub(crate) fn tool_grant(&self) -> Vec<String> {
        tool_names(&self.tools)
    }

    #[must_use]
    /// Runs the child detached.
    pub const fn in_background(mut self, background: bool) -> Self {
        self.background = background;
        self
    }
}

/// The names of `definitions`; one without a string `name` grants nothing.
pub(crate) fn tool_names(definitions: &[Value]) -> Vec<String> {
    definitions
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .map(str::to_owned)
        .collect()
}
