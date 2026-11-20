//! Cross-session memory as composable capability pieces.
//!
//! Nothing here touches the agent loop: saving and recalling are tools on the
//! tool seam, and injection is an admission decorator on the surface boundary.
//! The operator decides which pieces are wired and when they trigger.

#![deny(missing_docs)]
use std::sync::Arc;

use runtime::Memories;

mod admission;
mod config;
mod error;
mod recall_tool;
mod save_tool;
mod scope;

pub use admission::MemoryAdmission;
pub use config::{Config, Scope};
pub use error::Error;
pub use recall_tool::RecallTool;
pub use save_tool::SaveTool;

/// The tool set `config` enables, ready for registry composition.
#[must_use]
pub fn tools<S: Memories + 'static>(config: &Config, store: &S) -> Vec<Arc<dyn agent::Tool>> {
    let mut tools: Vec<Arc<dyn agent::Tool>> = Vec::new();
    if config.save {
        tools.push(Arc::new(SaveTool::new(store.clone(), config.scope)));
    }
    if config.recall_tool {
        tools.push(Arc::new(RecallTool::new(
            store.clone(),
            config.scope,
            config.recall_limit,
        )));
    }
    tools
}
