//! Tools that change while the process runs, such as one MCP server's catalog.

use std::pin::Pin;
use std::sync::Arc;

use crate::Tool;

/// Tools that change while the process runs. `refresh` runs before each claimed
/// operation's first model request, so it must bound how long it takes.
pub trait ToolSource: Send + Sync {
    /// The tools from the last successful listing.
    fn tools(&self) -> Vec<Arc<dyn Tool>>;

    /// List again. A failed listing keeps the tools listed before.
    fn refresh(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}
