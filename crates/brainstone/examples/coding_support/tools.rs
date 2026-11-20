use super::workspace::Workspace;
use brainstone::runtime::DurableStore;
use brainstone::{Registry, ToolError};

#[path = "checks.rs"]
mod checks;
#[path = "edit.rs"]
mod edit;
#[path = "guard.rs"]
mod guard;
#[path = "process.rs"]
mod process;
#[path = "read.rs"]
mod read;

pub(crate) fn registry(
    workspace: Workspace,
    store: DurableStore,
    crash: bool,
) -> Result<Registry, ToolError> {
    Ok(Registry::default()
        .with_tool(read::tool(workspace.clone())?)
        .with_tool(edit::tool(workspace.clone(), store, crash)?)
        .with_tool(checks::tool(workspace)?))
}
