//! A process adapter for evals/sqlite, not a deployment preset.
#[path = "eval_support/ledger.rs"]
mod ledger;
#[path = "eval_support/model.rs"]
mod model;
#[path = "eval_support/protocol.rs"]
mod protocol;
#[path = "eval_support/setup.rs"]
mod setup;
#[path = "eval_support/tool.rs"]
mod tool;

use brainstone::runtime::{Config, OperationId, SqliteStore};
use brainstone::{Address, Host, Prompt, Runtime};
use protocol::{Action, Input, emit};
use serde_json::json;
use std::time::Duration;

type Error = Box<dyn std::error::Error>;
const LEASE: Duration = Duration::from_secs(2);

#[tokio::main]
async fn main() -> Result<(), Error> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: eval_sqlite <input.json>")?;
    let input: Input = serde_json::from_slice(&std::fs::read(path)?)?;
    if let Err(error) = run(input).await {
        emit(
            &json!({"event":"driver_error", "code":"eval_driver_failed", "message":error.to_string()}),
        );
        return Err(error);
    }
    Ok(())
}

async fn run(input: Input) -> Result<(), Error> {
    let store = SqliteStore::connect(&format!("sqlite://{}", input.database)).await?;
    let runtime = Runtime::with_config(store.clone(), Config::new("eval", LEASE)?);
    if input.action == Action::Inspect {
        let id = OperationId::new(input.operation_id.ok_or("inspect requires operation_id")?)?;
        protocol::terminal(runtime.operation(id).await?.ok_or("operation missing")?);
        store.close().await;
        return Ok(());
    }
    let pool = ledger::open(&input.effects_database).await?;
    let host = setup::host(runtime, &input, pool.clone())?;
    let outcome = execute(&host, &input).await;
    host.shutdown().await;
    pool.close().await;
    store.close().await;
    outcome
}

async fn execute(host: &Host<SqliteStore, model::EvalModel>, input: &Input) -> Result<(), Error> {
    let id = if input.action == Action::Resume {
        OperationId::new(input.operation_id.ok_or("resume requires operation_id")?)?
    } else {
        let address = Address::new("eval", &input.conversation, "main")?;
        let profile = setup::profile(input)?;
        let prompt = Prompt::new(address, &input.request_id, &input.system, &input.text)?
            .with_profile(profile);
        let accepted = host.accept(&prompt).await?;
        emit(
            &json!({"event":"accepted", "operation_id":accepted.operation.id.get(), "duplicate":accepted.duplicate}),
        );
        if input.fault.as_deref() == Some("after_accept") {
            std::process::exit(protocol::CRASH);
        }
        accepted.operation.id
    };
    match host.execute(id).await {
        Ok(operation) => protocol::terminal(operation),
        Err(error) => emit(&json!({"event":"driver_error", "code":error.code()})),
    }
    Ok(())
}
