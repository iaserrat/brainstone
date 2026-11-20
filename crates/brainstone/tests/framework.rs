//! Rust composition preserves completed tool work and terminal outcomes across exits.
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use brainstone::agent::{
    Message, ModelResponse, Outcome, StopReason, Thread, ToolCall, ToolError, ToolOutput,
    ToolRequest,
};
use brainstone::runtime::{Address, Config, Prompt, SqliteStore};
use brainstone::{Host, Model, Registry, Runtime, Tool};
use serde_json::{Value, json};

const CRASH_CODE: i32 = 73;
const LEASE: Duration = Duration::from_millis(100);

struct Conversation {
    crash: bool,
}

impl Model for Conversation {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "Deterministic local model has no I/O."
    )]
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        match thread.last() {
            Message::ToolResults(results) => {
                assert_eq!(results[0].content, "durable ☕");
                if self.crash {
                    std::process::exit(CRASH_CODE);
                }
                response("recovered ☕", Vec::new(), StopReason::EndTurn)
            }
            Message::User(_) => response(
                "",
                vec![ToolCall {
                    id: "native-call".to_owned(),
                    name: "Remember".to_owned(),
                    args: json!({}),
                }],
                StopReason::ToolUse,
            ),
            other => panic!("unexpected history: {other:?}"),
        }
    }
}

fn response(text: &str, tool_calls: Vec<ToolCall>, stop: StopReason) -> ModelResponse {
    ModelResponse {
        reasoning: Vec::new(),
        text: text.to_owned(),
        tool_calls,
        stop,
        model: Some("local".to_owned()),
    }
}

struct Remember {
    permitted: bool,
}

impl Tool for Remember {
    fn name(&self) -> &'static str {
        "Remember"
    }
    fn definition(&self) -> Value {
        json!({"name": self.name(), "description": "Remember a value", "input_schema": {"type": "object"}})
    }
    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            assert!(self.permitted, "committed tool must not run again");
            assert!(request.idempotency_key.is_some());
            Ok(ToolOutput::from("durable ☕".to_owned()))
        })
    }
}

fn prompt() -> Prompt {
    Prompt::new(
        Address::new("test", "framework", "main").unwrap(),
        "request",
        "Use the native tool.",
        "remember ☕",
    )
    .unwrap()
}

fn host(store: SqliteStore, crash: bool) -> Host<SqliteStore, Conversation> {
    let runtime = Runtime::with_config(store, Config::new("framework", LEASE).unwrap());
    let tools = Registry::new(vec![Arc::new(Remember { permitted: crash })]);
    Host::new(runtime, Conversation { crash }, tools)
}

#[test]
fn rust_composition_recovers_committed_tool_work_after_process_exit() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}",
        directory.path().join("framework.db").display()
    );
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "framework_crash_child", "--ignored"])
        .env("BRAINSTONE_FRAMEWORK_CRASH_URL", &url)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(CRASH_CODE));
    std::thread::sleep(LEASE * 2);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(recover(&url));
    runtime.block_on(retry(&url));
}

async fn recover(url: &str) {
    let store = SqliteStore::connect(url).await.unwrap();
    let host = host(store.clone(), false);
    let accepted = host.accept(&prompt()).await.unwrap();
    assert!(accepted.duplicate);
    let operation = host.execute(accepted.operation.id).await.unwrap();
    assert_eq!(
        operation.outcome,
        Some(Outcome::Completed {
            text: "recovered ☕".to_owned()
        })
    );
    host.shutdown().await;
    assert_eq!(
        host.accept(&prompt()).await.unwrap_err().code(),
        "host_shutting_down"
    );
    store.close().await;
}

struct NeverGenerate;
impl Model for NeverGenerate {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "A terminal retry never reaches inference."
    )]
    async fn generate(&self, _: &Thread, _: &[Value]) -> ModelResponse {
        panic!("terminal retry must not call the model")
    }
}

async fn retry(url: &str) {
    let store = SqliteStore::connect(url).await.unwrap();
    let host = Host::new(
        Runtime::new(store.clone()),
        NeverGenerate,
        Registry::default(),
    );
    let accepted = host.accept(&prompt()).await.unwrap();
    assert!(accepted.duplicate);
    let operation = host.execute(accepted.operation.id).await.unwrap();
    assert_eq!(
        operation.outcome,
        Some(Outcome::Completed {
            text: "recovered ☕".to_owned()
        })
    );
    host.shutdown().await;
    store.close().await;
}

#[tokio::test]
#[ignore = "subprocess crash fixture"]
async fn framework_crash_child() {
    let url = std::env::var("BRAINSTONE_FRAMEWORK_CRASH_URL").unwrap();
    let store = SqliteStore::connect(&url).await.unwrap();
    let host = host(store, true);
    let accepted = host.accept(&prompt()).await.unwrap();
    host.execute(accepted.operation.id).await.unwrap();
    panic!("the child must exit after the committed tool result")
}
