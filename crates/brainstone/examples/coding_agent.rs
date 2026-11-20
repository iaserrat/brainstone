//! A repository-local coding agent with typed tools and durable operator approvals.
#[path = "coding_support/approval.rs"]
mod approval;
#[path = "coding_support/console.rs"]
mod console;
#[path = "coding_support/demo.rs"]
mod demo;
#[path = "coding_support/input.rs"]
mod input;
#[path = "coding_support/model.rs"]
mod model;
#[path = "coding_support/profile.rs"]
mod profile;
#[path = "coding_support/provider.rs"]
mod provider;
#[path = "coding_support/run.rs"]
mod run;
#[path = "coding_support/state.rs"]
mod state;
#[path = "coding_support/tools.rs"]
mod tools;
#[path = "coding_support/workspace.rs"]
mod workspace;

use brainstone::runtime::{Config, DurableStore, Operation};
use brainstone::{Host, Model, Registry, Runtime};
use std::time::Duration;
type Error = Box<dyn std::error::Error>;
const LEASE_TTL: Duration = Duration::from_secs(30);
const CRASH_LEASE_TTL: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> Result<(), Error> {
    execute(input::options()?).await
}
async fn execute(options: input::Options) -> Result<(), Error> {
    if options.action.is_demo() {
        demo::prepare(&options.root)?;
    }
    let workspace = workspace::Workspace::open(&options.root)?;
    let url = state::database(workspace.root())?;
    let store = DurableStore::connect(&url).await?;
    let result = dispatch(&store, workspace, options.action).await;
    store.close().await;
    result
}
async fn dispatch(
    store: &DurableStore,
    workspace: workspace::Workspace,
    action: input::Action,
) -> Result<(), Error> {
    let tools = tools::registry(workspace, store.clone(), action.crashes())?;
    if action.is_demo() {
        return drive(store, tools, action, model::LocalModel).await;
    }
    drive(store, tools, action, provider::live()?).await
}
async fn drive<M: Model>(
    store: &DurableStore,
    tools: Registry,
    action: input::Action,
    model: M,
) -> Result<(), Error> {
    let ttl = if action.crashes() {
        CRASH_LEASE_TTL
    } else {
        LEASE_TTL
    };
    let config = Config::new("coding-example", ttl)?;
    let host = Host::new(Runtime::with_config(store.clone(), config), model, tools);
    let result = execute_operation(&host, store, &action).await;
    host.shutdown().await;
    run::show(result?)
}
async fn execute_operation<M: Model>(
    host: &Host<DurableStore, M>,
    store: &DurableStore,
    action: &input::Action,
) -> Result<Operation, Error> {
    let id = if let input::Action::Resume(id) = action {
        *id
    } else {
        let accepted = host.accept(&run::prompt(action)?).await?;
        eprintln!(
            "operation={} duplicate={}",
            accepted.operation.id.get(),
            accepted.duplicate
        );
        accepted.operation.id
    };
    approval::execute(store, id, action.is_demo(), host.execute(id)).await
}

#[cfg(test)]
#[path = "coding_support/coding_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "coding_support/provider_tests.rs"]
mod provider_tests;
