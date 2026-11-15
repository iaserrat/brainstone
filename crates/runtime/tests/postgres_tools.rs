#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Opt-in contract tests: lanes written before this release offer the tools of
//! the registry that runs them, and a subagent keeps its spawn grant.

use std::pin::Pin;
use std::sync::{Arc, Mutex};

use agent::{
    ExecutionProfile, Model, ModelResponse, Registry, StopReason, Thread, Tool, ToolError,
    ToolOutput, ToolRequest,
};
use brainstone_runtime::{
    Address, OperationId, PostgresStore, Prompt, Runtime, SubagentConfig, SubagentRequest,
    SubagentStore,
};
use serde_json::{Value, json};

#[derive(Default)]
struct Offered(Mutex<Vec<String>>);

impl Model for Offered {
    async fn generate(&self, _thread: &Thread, tools: &[Value]) -> ModelResponse {
        *self.0.lock().unwrap() = tools
            .iter()
            .map(|tool| {
                format!(
                    "{}:{}",
                    tool["name"].as_str().unwrap(),
                    tool["description"].as_str().unwrap()
                )
            })
            .collect();
        ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: "done".to_owned(),
            tool_calls: Vec::new(),
            stop: StopReason::EndTurn,
        }
    }
}

struct Named(&'static str, &'static str);

impl Tool for Named {
    fn name(&self) -> &str {
        self.0
    }

    fn definition(&self) -> Value {
        json!({"name": self.0, "description": self.1, "input_schema": {"type": "object"}})
    }

    fn call<'a>(
        &'a self,
        _request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async { Ok(String::new().into()) })
    }
}

fn registry(tools: &[(&'static str, &'static str)]) -> Registry {
    Registry::new(
        tools
            .iter()
            .map(|(name, description)| -> Arc<dyn Tool> { Arc::new(Named(name, description)) })
            .collect(),
    )
}

fn said(conversation: &str, request: &str) -> Prompt {
    let address = Address::new("contract", conversation, "tools").unwrap();
    Prompt::new(address, request, "system", "héllo ☕").unwrap()
}

fn suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("tools-{}-{nanos}", std::process::id())
}

async fn run(store: &PostgresStore, id: OperationId, tools: &Registry) -> Vec<String> {
    let model = Offered::default();
    Runtime::new(store.clone())
        .run_operation(id, &model, tools)
        .await
        .unwrap()
        .expect("claimed");
    model.0.into_inner().unwrap()
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn an_existing_lane_offers_tools_added_after_it_was_created() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let conversation = suffix();
    let before = registry(&[("search", "v1")]);
    let store = PostgresStore::connect(&url).await.unwrap();
    let first = Runtime::new(store.clone())
        .accept_with_tools(&said(&conversation, "m1"), &before)
        .await
        .unwrap();
    run(&store, first.operation.id, &before).await;
    store.close().await;

    let deployed = registry(&[("search", "v2"), ("create", "v1")]);
    let store = PostgresStore::connect(&url).await.unwrap();
    let next = Runtime::new(store.clone())
        .accept_with_tools(&said(&conversation, "m2"), &deployed)
        .await
        .unwrap();

    let offered = run(&store, next.operation.id, &deployed).await;
    assert_eq!(offered, ["search:v2", "create:v1"]);
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn a_subagent_offers_only_its_spawn_grant_at_current_schemas() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.unwrap();
    let parent = Runtime::new(store.clone())
        .accept(&said(&suffix(), "parent"))
        .await
        .unwrap();
    let grant = vec![Named("search", "v1").definition()];
    let config = SubagentConfig::new("reviewer", "review", ExecutionProfile::default())
        .unwrap()
        .with_tools(grant);
    let request = SubagentRequest::new(parent.operation.id, "spawn", "task", config).unwrap();
    let child = store.spawn_subagent(&request).await.unwrap().agent;

    let deployed = registry(&[("search", "v2"), ("create", "v1")]);
    let offered = run(&store, child.operation.id, &deployed).await;

    assert_eq!(offered, ["search:v2"]);
    store.close().await;
}
