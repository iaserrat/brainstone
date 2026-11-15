//! A durable agent with no network: an in-process echo model, one custom
//! tool, and a SQLite database that survives restarts.
//!
//! ```sh
//! cargo run -p brainstone --example echo        # runs the turn
//! cargo run -p brainstone --example echo        # replays the same outcome
//! ```
//!
//! Every step is a durable record. `accept` stores the prompt before any
//! model runs; a second run of this example gets the same committed outcome
//! back instead of running again. Delete `echo-agent.db` to start fresh.
use std::pin::Pin;
use std::sync::Arc;

use brainstone::agent::{
    Message, Model, ModelResponse, Outcome, Registry, StopReason, Thread, Tool, ToolError,
    ToolOutput, ToolRequest,
};
use brainstone::runtime::{Address, Prompt, Runtime, SqliteStore};
use serde_json::{Value, json};

/// Repeats the latest user message back. A real provider implements the same
/// [`Model`] seam against its network API and owns its own retries.
struct Echo;

impl Model for Echo {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "The seam is async; an in-process model simply never awaits."
    )]
    async fn generate(&self, thread: &Thread, _tools: &[Value]) -> ModelResponse {
        let text = match thread.last() {
            Message::User(text) => format!("echo: {text}"),
            _ => String::new(),
        };
        ModelResponse {
            reasoning: Vec::new(),
            text,
            tool_calls: Vec::new(),
            stop: StopReason::EndTurn,
            model: Some("echo".to_owned()),
        }
    }
}

/// A tool the registry offers the model. Arguments are JSON by contract and
/// failures carry a stable code, never free text.
struct UtcNow;

impl Tool for UtcNow {
    fn name(&self) -> &'static str {
        "UtcNow"
    }

    fn definition(&self) -> Value {
        json!({
            "name": self.name(),
            "description": "The current UTC instant as a Unix millisecond count.",
            "input_schema": {"type": "object", "properties": {}, "additionalProperties": false}
        })
    }

    fn call<'a>(
        &'a self,
        _request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or("unknown".to_owned(), |age| age.as_millis().to_string());
            Ok(ToolOutput::from(format!("unix-ms:{now}")))
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = SqliteStore::connect("sqlite://echo-agent.db?mode=rwc").await?;
    let runtime = Runtime::new(store);
    let address = Address::new("demo", "conversation-42", "main")?;
    let system = "You repeat what the user sends.";
    let prompt = Prompt::new(address, "request-1", system, "hello durable world")?;

    // Accepting twice returns the stored operation instead of a new one: the
    // (lane, request_id) pair is the idempotency boundary.
    let accepted = runtime.accept(&prompt).await?;
    let mut outcome = accepted.operation.outcome;
    if outcome.is_some() {
        println!("already stored (duplicate: {})", accepted.duplicate);
    }
    if outcome.is_none() {
        let tools = Registry::default().extend(vec![Arc::new(UtcNow)]);
        outcome = runtime
            .run_operation(accepted.operation.id, &Echo, &tools)
            .await?
            .and_then(|operation| operation.outcome);
    }

    match outcome {
        Some(Outcome::Completed { text }) => println!("{text}"),
        other => println!("no settled outcome: {other:?}"),
    }
    Ok(())
}
