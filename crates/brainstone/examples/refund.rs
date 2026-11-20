//! Simulated refunds only: no payment service or model API is contacted.
#[path = "refund_support/approval.rs"]
mod approval;
#[path = "refund_support/input.rs"]
mod input;
#[path = "refund_support/ledger.rs"]
mod ledger;
#[path = "refund_support/model.rs"]
mod model;
#[path = "refund_support/tool.rs"]
mod tool;

use brainstone::runtime::{Config, DurableStore};
use brainstone::{Address, ExecutionProfile, Host, Prompt, Registry, Runtime, ToolContract};
use std::time::Duration;

type Error = Box<dyn std::error::Error>;
const LEASE_TTL: Duration = Duration::from_secs(30);
const CRASH_LEASE_TTL: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> Result<(), Error> {
    execute(input::options()?).await
}

async fn execute(options: input::Options) -> Result<(), Error> {
    let store = DurableStore::connect(&options.url).await?;
    let pool = ledger::open(&options.url).await?;
    let result = run(&store, pool.clone(), options.crash).await;
    pool.close().await;
    store.close().await;
    result
}

async fn run(store: &DurableStore, pool: sqlx::SqlitePool, crash: bool) -> Result<(), Error> {
    let tools = Registry::default().with_tool(tool::tool(pool, crash)?);
    let ttl = if crash { CRASH_LEASE_TTL } else { LEASE_TTL };
    let config = Config::new("refund-example", ttl)?;
    let host = Host::new(
        Runtime::with_config(store.clone(), config),
        model::LocalModel,
        tools,
    );
    let accepted = host.accept(&prompt()?).await?;
    let result = approval::execute(
        store,
        accepted.operation.id,
        host.execute(accepted.operation.id),
    )
    .await;
    host.shutdown().await;
    println!("{:?}", result?.outcome);
    Ok(())
}

fn prompt() -> Result<Prompt, Error> {
    let contract = ToolContract::only(["Refund"])?
        .require_approval(["Refund"])?
        .require_success(["Refund"])?;
    let profile = ExecutionProfile::new(None, None, 3, 0)?.with_tool_contract(contract);
    Ok(Prompt::new(
        Address::new("demo", "refund", "main")?,
        "refund-1",
        "Use Refund to perform the simulated refund.",
        "Refund 500 cents for BS-1001.",
    )?
    .with_profile(profile))
}

#[cfg(test)]
#[path = "refund_support/refund_tests.rs"]
mod tests;
