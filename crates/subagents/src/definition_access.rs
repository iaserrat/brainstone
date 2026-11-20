use std::path::Path;

use crate::{Definition, Effort, Model};

impl Definition {
    /// The agent's name, unique within the catalog.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// The one-line description the parent's tool list shows.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
    /// The child's system prompt, with frontmatter removed.
    #[must_use]
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }
    /// The allowlist of tool names to offer, or `None` for the parent's set.
    #[must_use]
    pub fn tools(&self) -> Option<&[String]> {
        self.tools.as_deref()
    }
    /// Tool names removed from whatever set is offered.
    #[must_use]
    pub fn disallowed_tools(&self) -> &[String] {
        &self.disallowed_tools
    }
    /// Which model the child runs.
    #[must_use]
    pub const fn model(&self) -> &Model {
        &self.model
    }
    /// Omission inherits the parent; `Unlimited` explicitly removes its limit.
    #[must_use]
    pub const fn max_turns(&self) -> Option<agent::TurnLimit> {
        self.max_turns
    }
    /// The child's reasoning effort, or the parent's when unset.
    #[must_use]
    pub const fn effort(&self) -> Option<Effort> {
        self.effort
    }
    /// Whether the child runs detached, or the parent's default when unset.
    #[must_use]
    pub const fn background(&self) -> Option<bool> {
        self.background
    }
    /// The MCP servers the child may use.
    #[must_use]
    pub fn mcp_servers(&self) -> &[String] {
        &self.mcp_servers
    }
    /// The file the definition was read from.
    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source
    }
}
