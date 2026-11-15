//! Table-driven ownership cases and `PostgreSQL` crash/reopen boundaries.
#![allow(clippy::too_many_lines)] // Case bodies show the complete transitions.

use std::path::Path;
use std::time::Duration;

use agent::{
    ExecutionProfile, Message, Model, ModelResponse, Registry, StopReason, Thread, ToolCall,
};
use brainstone_subagents::{Catalog, Control};
use runtime::{
    Address, Config, MemoryStore, OperationId, OperationState, PostgresStore, Prompt, Runtime,
    Store, Subagent, SubagentConfig, SubagentRequest, SubagentStore,
};
use serde_json::{Value, json};

const LEASE_TTL: Duration = Duration::from_millis(300);
const RECOVERY_WAIT: Duration = Duration::from_millis(400);
const SLOW_RESPONSE: Duration = Duration::from_secs(3);
const WAIT_TOLERANCE: Duration = Duration::from_secs(2);

struct Answer(Duration);

impl Model for Answer {
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        assert_eq!(thread.last(), &Message::User("start".to_owned()));
        tokio::time::sleep(self.0).await;
        ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: "done".to_owned(),
            tool_calls: Vec::new(),
            stop: StopReason::EndTurn,
        }
    }
}

#[tokio::test]
async fn task_output_honors_timeout_with_either_child_owner() {
    for (claimed, block, expected) in [
        (true, true, "running"),
        (false, true, "running"),
        (false, false, "accepted"),
    ] {
        let store = MemoryStore::default();
        let runtime = runtime(store.clone());
        let (parent, child) = child(&runtime).await;
        if claimed {
            store
                .claim_operation(child.operation.id, "other", Duration::from_secs(30))
                .await
                .unwrap()
                .unwrap();
        }
        let tools = controls(runtime);
        let start = tokio::time::Instant::now();
        let result = output(&tools, parent, child.id, block).await;
        assert!(
            start.elapsed() < WAIT_TOLERANCE,
            "one-second timeout exceeded two seconds"
        );
        assert_eq!(result["state"], expected);
        assert!(result["outcome"].is_null());
        assert_eq!(result["agent_id"], child.id.get());
    }
}

#[tokio::test]
async fn completed_output_returns_the_durable_outcome_immediately() {
    let runtime = runtime(MemoryStore::default());
    let (parent, child) = child(&runtime).await;
    runtime
        .run_operation(
            child.operation.id,
            &Answer(Duration::ZERO),
            &Registry::default(),
        )
        .await
        .unwrap()
        .unwrap();
    let tools = controls(runtime);
    let start = tokio::time::Instant::now();
    let result = output(&tools, parent, child.id, true).await;
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(result["state"], "completed");
    assert_eq!(result["outcome"]["Completed"]["text"], "done");
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn a_timed_out_inline_child_resumes_after_postgres_reopens() {
    let store = postgres().await;
    let runtime = runtime(store.clone());
    let (parent, child) = child(&runtime).await;
    let tools = controls(runtime);
    let start = tokio::time::Instant::now();
    let result = output(&tools, parent, child.id, true).await;
    assert!(start.elapsed() < WAIT_TOLERANCE);
    assert_eq!(result["state"], "running");
    assert!(result["outcome"].is_null());
    store.close().await;
    drop(tools);
    tokio::time::sleep(RECOVERY_WAIT).await;
    let reopened = postgres().await;
    let recovery = self::runtime(reopened.clone());
    let operation = recovery
        .run_operation(
            child.operation.id,
            &Answer(Duration::ZERO),
            &Registry::default(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(operation.state, OperationState::Completed);
    assert_eq!(
        operation.outcome,
        Some(agent::Outcome::Completed {
            text: "done".to_owned()
        })
    );
    recovery
        .run_operation(parent, &Answer(Duration::ZERO), &Registry::default())
        .await
        .unwrap()
        .unwrap();
    reopened.close().await;
}

async fn postgres() -> PostgresStore {
    PostgresStore::connect(&std::env::var("BRAINSTONE_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap()
}

fn runtime<S: Store>(store: S) -> Runtime<S> {
    Runtime::with_config(store, Config::new("output-wait", LEASE_TTL).unwrap())
}

async fn child<S: Store + SubagentStore>(runtime: &Runtime<S>) -> (OperationId, Subagent) {
    let conversation = format!("output-timeout-{}", std::process::id());
    let address = Address::new("test", &conversation, "main").unwrap();
    let prompt = Prompt::new(address, "parent", "system", "start").unwrap();
    let parent = runtime.accept(&prompt).await.unwrap().operation.id;
    let config = SubagentConfig::new("child", "child system", ExecutionProfile::default()).unwrap();
    let request = SubagentRequest::new(parent, "spawn", "start", config).unwrap();
    (
        parent,
        runtime.spawn_subagent(&request).await.unwrap().agent,
    )
}

fn controls<S: Store + SubagentStore + 'static>(runtime: Runtime<S>) -> Registry {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/control");
    Control::new(
        runtime,
        Catalog::load(&root).unwrap(),
        Registry::default(),
        Answer(SLOW_RESPONSE),
    )
    .registry()
}

async fn output(
    tools: &Registry,
    parent: OperationId,
    child: runtime::SubagentId,
    block: bool,
) -> Value {
    let call = ToolCall {
        id: "output".to_owned(),
        name: "TaskOutput".to_owned(),
        args: json!({"agent_id":child.get(), "timeout_seconds":1, "block":block}),
    };
    let context = agent::ToolContext {
        identity: None,
        operation_id: u64::try_from(parent.get()).unwrap(),
        profile: ExecutionProfile::default(),
    };
    let result = agent::execute_tool_with_context(&call, tools, None, Some(context)).await;
    assert!(!result.is_error, "{}", result.content);
    serde_json::from_str(&result.content).unwrap()
}
