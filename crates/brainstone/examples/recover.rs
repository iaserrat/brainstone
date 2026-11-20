//! A custom model and native tool recover without environment configuration.
//!
//! Run `cargo run -p brainstone --no-default-features --example recover -- --crash`.
//! It exits after the tool result commits. Run again without `--crash` to resume;
//! subsequent runs return the same durable outcome without inference or tool calls.
//! Pass a SQLite URL after the optional flag to select a fresh file and repeat.
use brainstone::agent::{
    Message, ModelResponse, StopReason, Thread, ToolCall, ToolError, ToolOutput, ToolRequest,
};
use brainstone::runtime::{Address, Config, Prompt, SqliteStore};
use brainstone::{Host, Model, Registry, Runtime, Tool};
use serde_json::{Value, json};
use std::{pin::Pin, sync::Arc, time::Duration};

const CRASH_CODE: i32 = 73;
const LEASE_TTL: Duration = Duration::from_secs(30);
const CRASH_LEASE_TTL: Duration = Duration::from_secs(5);

struct LocalModel {
    crash: bool,
}
impl Model for LocalModel {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "The example model is deterministic and local."
    )]
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        let mut response = ModelResponse {
            reasoning: Vec::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            stop: StopReason::EndTurn,
            model: Some("local".to_owned()),
        };
        if matches!(thread.last(), Message::User(_)) {
            response.stop = StopReason::ToolUse;
            response.tool_calls.push(ToolCall {
                id: "remember".to_owned(),
                name: "Remember".to_owned(),
                args: json!({}),
            });
        } else if self.crash {
            crash_after_tool_result();
        } else {
            response
                .text
                .push_str("Recovered the committed tool result.");
        }
        response
    }
}

fn crash_after_tool_result() -> ! {
    eprintln!("Tool result committed; exiting before the final answer. Restart without --crash.");
    std::process::exit(CRASH_CODE);
}

struct Remember;
impl Tool for Remember {
    fn name(&self) -> &'static str {
        "Remember"
    }
    fn definition(&self) -> Value {
        json!({"name": self.name(), "description": "Return a durable value", "input_schema": {"type": "object"}})
    }
    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            println!(
                "Native tool called with retry key {:?}",
                request.idempotency_key
            );
            Ok(ToolOutput::from("remembered ☕".to_owned()))
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    let crash = first.as_deref() == Some("--crash");
    let url = if crash { args.next() } else { first }
        .unwrap_or_else(|| "sqlite://recover-agent.db".to_owned());
    if args.next().is_some() {
        return Err("usage: recover [--crash] [sqlite-url]".into());
    }
    let store = SqliteStore::connect(&url).await?;
    let ttl = if crash { CRASH_LEASE_TTL } else { LEASE_TTL };
    let config = Config::new("recover-example", ttl)?;
    let runtime = Runtime::with_config(store.clone(), config);
    let tools = Registry::new(vec![Arc::new(Remember)]);
    let host = Host::new(runtime, LocalModel { crash }, tools);
    let address = Address::new("example", "recovery", "main")?;
    let prompt = Prompt::new(address, "request-1", "Remember with the tool.", "Hello")?;
    let accepted = host.accept(&prompt).await?;
    let operation = host.execute(accepted.operation.id).await?;
    println!("{:?}", operation.outcome);
    host.shutdown().await;
    store.close().await;
    Ok(())
}
