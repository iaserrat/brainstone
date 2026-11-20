#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
use std::path::Path;
use std::sync::{Arc, Mutex};

use agent::{Effort, Message, Model, ModelResponse, Registry, StopReason, Thread, ToolCall};
use brainstone_subagents::{Catalog, Control};
use runtime::{Address, MemoryStore, Prompt, Runtime};

#[derive(Default)]
struct State {
    child_profile: Mutex<Option<agent::ExecutionProfile>>,
}

#[derive(Clone, Default)]
struct Delegating(Arc<State>);

#[derive(Clone, Default)]
struct Background(Arc<Mutex<bool>>);

impl Model for Delegating {
    async fn generate(&self, thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        if thread.system == "Review independently.\n" {
            self.0
                .child_profile
                .lock()
                .unwrap()
                .replace(thread.profile().clone());
            return answer("child report");
        }
        if matches!(thread.last(), Message::User(_)) {
            return ModelResponse {
                model: None,
                reasoning: Vec::new(),
                text: String::new(),
                tool_calls: vec![ToolCall {
                    id: "spawn-1".to_owned(),
                    name: "Agent".to_owned(),
                    args: serde_json::json!({"subagent_type":"reviewer","prompt":"review this","run_in_background":false}),
                }],
                stop: StopReason::ToolUse,
            };
        }
        answer("parent received report")
    }
}

impl Model for Background {
    async fn generate(&self, thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        if thread.system == "Review independently.\n" {
            return answer("background report");
        }
        if matches!(thread.last(), Message::User(text) if text.contains("subagent_completion")) {
            *self.0.lock().unwrap() = true;
            return answer("notification handled");
        }
        if matches!(thread.last(), Message::User(_)) {
            return ModelResponse {
                model: None,
                reasoning: Vec::new(),
                text: String::new(),
                tool_calls: vec![ToolCall {
                    id: "background-1".to_owned(),
                    name: "Agent".to_owned(),
                    args: serde_json::json!({"subagent_type":"reviewer","prompt":"review","run_in_background":true}),
                }],
                stop: StopReason::ToolUse,
            };
        }
        answer("parent continued")
    }
}

#[tokio::test]
async fn foreground_agent_runs_to_completion_without_an_external_worker() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/control");
    let catalog = Catalog::load(&root).unwrap();
    let store = MemoryStore::default();
    let runtime = Runtime::new(store);
    let model = Delegating::default();
    let tools =
        Control::new(runtime.clone(), catalog, Registry::default(), model.clone()).registry();
    let address = Address::new("test", "conversation", "main").unwrap();
    let prompt = Prompt::new(address, "request", "parent system", "delegate").unwrap();
    runtime.accept_with_tools(&prompt, &tools).await.unwrap();

    let operation = runtime.run_next(&model, &tools).await.unwrap().unwrap();

    assert!(
        matches!(operation.outcome, Some(agent::Outcome::Completed { ref text }) if text == "parent received report")
    );
    let profile = model
        .0
        .child_profile
        .lock()
        .unwrap()
        .clone()
        .expect("child ran");
    assert_eq!(profile.model(), Some("claude-sonnet-5"));
    assert_eq!(profile.effort(), Some(Effort::Low));
    assert_eq!(profile.max_turns(), Some(7));
    assert_eq!(profile.depth(), 1);
}

#[tokio::test]
async fn an_agent_without_max_turns_inherits_its_parent_budget() {
    assert_child_limit("inherit", Some(42), Some(42)).await;
}

#[tokio::test]
async fn an_agent_without_max_turns_inherits_unlimited_execution() {
    assert_child_limit("inherit", None, None).await;
}

#[tokio::test]
async fn an_agent_can_explicitly_remove_an_inherited_limit() {
    assert_child_limit("unlimited", Some(42), None).await;
}

async fn assert_child_limit(fixture: &str, parent: Option<usize>, expected: Option<usize>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture);
    let store = MemoryStore::default();
    let runtime = Runtime::new(store);
    let model = Delegating::default();
    let tools = Control::new(
        runtime.clone(),
        Catalog::load(&root).unwrap(),
        Registry::default(),
        model.clone(),
    )
    .registry();
    let address = Address::new("test", "inherit", "main").unwrap();
    let profile = agent::ExecutionProfile::new(None, None, parent, 0).unwrap();
    let prompt = Prompt::new(address, "request", "parent system", "delegate")
        .unwrap()
        .with_profile(profile);
    runtime.accept_with_tools(&prompt, &tools).await.unwrap();

    runtime.run_next(&model, &tools).await.unwrap().unwrap();

    let profile = model.0.child_profile.lock().unwrap().clone().unwrap();
    assert_eq!(profile.max_turns(), expected);
}

#[tokio::test]
async fn foreground_only_control_finishes_a_requested_background_agent_inline() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/control");
    let runtime = Runtime::new(MemoryStore::default());
    let model = Background::default();
    let tools = Control::foreground(
        runtime.clone(),
        Catalog::load(&root).unwrap(),
        Registry::default(),
        model.clone(),
    )
    .registry();
    let address = Address::new("test", "one-shot", "main").unwrap();
    let prompt = Prompt::new(address, "request", "parent system", "delegate").unwrap();
    runtime.accept_with_tools(&prompt, &tools).await.unwrap();

    let operation = runtime.run_next(&model, &tools).await.unwrap().unwrap();

    assert!(matches!(
        operation.outcome,
        Some(agent::Outcome::Completed { ref text }) if text == "parent continued"
    ));
    assert!(runtime.run_next(&model, &tools).await.unwrap().is_none());
}

#[tokio::test]
async fn background_completion_becomes_a_durable_parent_turn() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/control");
    let runtime = Runtime::new(MemoryStore::default());
    let model = Background::default();
    let tools = Control::new(
        runtime.clone(),
        Catalog::load(&root).unwrap(),
        Registry::default(),
        model.clone(),
    )
    .registry();
    let address = Address::new("test", "conversation", "main").unwrap();
    let prompt = Prompt::new(address, "request", "parent system", "delegate").unwrap();
    runtime.accept_with_tools(&prompt, &tools).await.unwrap();

    runtime.run_next(&model, &tools).await.unwrap();
    runtime.run_next(&model, &tools).await.unwrap();
    let notification = runtime.run_next(&model, &tools).await.unwrap().unwrap();

    assert!(
        matches!(notification.outcome, Some(agent::Outcome::Completed { ref text }) if text == "notification handled")
    );
    assert!(*model.0.lock().unwrap());
}

#[test]
fn startup_rejects_an_agent_that_resolves_to_no_tools() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/no-tools");
    let control = Control::new(
        Runtime::new(MemoryStore::default()),
        Catalog::load(&root).unwrap(),
        Registry::default(),
        Delegating::default(),
    );

    assert_eq!(control.validate().unwrap_err().code(), "agent_has_no_tools");
}

#[tokio::test]
async fn task_output_rejects_an_out_of_range_timeout() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/control");
    let runtime = Runtime::new(MemoryStore::default());
    let parent = accept_parent(&runtime).await;
    let tools = Control::new(
        runtime,
        Catalog::load(&root).unwrap(),
        Registry::default(),
        Delegating::default(),
    )
    .registry();
    let call = ToolCall {
        id: "output".to_owned(),
        name: "TaskOutput".to_owned(),
        args: serde_json::json!({"agent_id":1,"timeout_seconds":0}),
    };
    let context = agent::ToolContext {
        identity: None,
        operation_id: u64::try_from(parent.operation.id.get()).unwrap(),
        profile: agent::ExecutionProfile::default(),
    };

    let result = agent::execute_tool_with_context(&call, &tools, None, Some(context)).await;

    assert!(result.is_error);
    assert!(result.content.contains("invalid_agent_arguments"));
}

async fn accept_parent(runtime: &Runtime<MemoryStore>) -> runtime::Accepted {
    let address = Address::new("test", "timeout", "main").unwrap();
    let prompt = Prompt::new(address, "parent", "system", "task").unwrap();
    runtime.accept(&prompt).await.unwrap()
}

fn answer(text: &str) -> ModelResponse {
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: text.to_owned(),
        tool_calls: Vec::new(),
        stop: StopReason::EndTurn,
    }
}
