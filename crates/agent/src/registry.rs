//! The per-run tool set.
//!
//! The definitions sent to the model and the table the loop dispatches through
//! are one object, so "advertised but uncallable" is unrepresentable.

use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::registry_select::selected;
use crate::tool::Tool;
use crate::tool_source::ToolSource;

#[derive(Clone)]
enum Entry {
    Tool(Arc<dyn Tool>),
    Source(Arc<dyn ToolSource>),
}

/// Fixed tools plus sources whose tools change while the process runs.
#[derive(Clone, Default)]
pub struct Registry {
    entries: Vec<Entry>,
}

impl Registry {
    #[must_use]
    /// A registry over a fixed set of tools.
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        Self {
            entries: tools.into_iter().map(Entry::Tool).collect(),
        }
    }

    /// Offers each source's current tools at once; a source lists before it is added.
    /// [`Registry::listed`] lists every source again.
    #[must_use]
    pub fn from_sources(sources: Vec<Arc<dyn ToolSource>>) -> Self {
        Self {
            entries: sources.into_iter().map(Entry::Source).collect(),
        }
    }

    /// List every source again, one after another, into a registry that never changes.
    /// A source that fails to list keeps the tools it listed before.
    pub async fn listed(&self) -> Self {
        for entry in &self.entries {
            if let Entry::Source(source) = entry {
                source.refresh().await;
            }
        }
        Self::new(self.tools())
    }

    /// The first tool registered under `name`, if any.
    #[must_use]
    // software-design-allow: shallow-function public; hides how sources are flattened
    pub fn lookup(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools().into_iter().find(|t| t.name() == name)
    }

    /// Every definition, in registration order, for the provider call.
    #[must_use]
    pub fn definitions(&self) -> Vec<Value> {
        self.tools().iter().map(|t| t.definition()).collect()
    }

    /// This registry followed by `tools`; its sources stay live.
    #[must_use]
    pub fn extend(&self, tools: Vec<Arc<dyn Tool>>) -> Self {
        let mut entries = self.entries.clone();
        entries.extend(tools.into_iter().map(Entry::Tool));
        Self { entries }
    }

    /// A fixed copy of the current tools `allow` admits (`None`: all, empty: none) minus `deny`.
    /// Patterns: `mcp__*`, `<prefix>__*`, `mcp__<server>`, or a name; unmatched ones are ignored.
    #[must_use]
    pub fn select(&self, allow: Option<&[String]>, deny: &[String]) -> Self {
        let tools = self
            .tools()
            .into_iter()
            .filter(|tool| selected(tool.name(), allow, deny))
            .collect();
        Self::new(tools)
    }

    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.entries
            .iter()
            .flat_map(|entry| match entry {
                Entry::Tool(tool) => vec![Arc::clone(tool)],
                Entry::Source(source) => source.tools(),
            })
            .collect()
    }
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tools = self.tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        f.debug_struct("Registry").field("tools", &names).finish()
    }
}
