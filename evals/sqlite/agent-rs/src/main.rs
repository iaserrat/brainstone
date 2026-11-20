//! Evaluation-only adapter. Reads one JSON request file and emits NDJSON evidence.
mod driver;
mod effects;
mod ledger;
mod model;
mod protocol;
mod state;

#[tokio::main]
async fn main() {
    if let Err(error) = execute().await {
        if let Some(error) = error.downcast_ref::<agent_rs::Error>() {
            protocol::error(error);
            std::process::exit(1);
        }
        protocol::emit(&serde_json::json!({"event":"driver_error", "code":"adapter_error"}));
        std::process::exit(1);
    }
}

async fn execute() -> Result<(), protocol::Error> {
    let path = std::env::args().nth(1).ok_or("input_file_required")?;
    let input = serde_json::from_slice(&std::fs::read(path)?)?;
    driver::run(input).await
}
