#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Which tools a claimed operation offers: the worker's registry, narrowed to
//! a subagent's spawn grant.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use agent::{
    ExecutionProfile, Message, Model, ModelResponse, Registry, StopReason, Thread, Tool, ToolCall,
    ToolError, ToolOutput, ToolRequest,
};
use serde_json::{Value, json};

use crate::{Address, MemoryStore, Prompt, Runtime, Store, SubagentConfig, SubagentRequest};

/// Records `name:description` of every offered tool per model request.
#[derive(Default)]
struct ToolNames(Mutex<Vec<Vec<String>>>);

impl ToolNames {
    fn last(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("a model request")
    }
}

impl Model for ToolNames {
    async fn generate(&self, _thread: &Thread, tools: &[Value]) -> ModelResponse {
        let offered = tools
            .iter()
            .map(|tool| {
                format!(
                    "{}:{}",
                    tool["name"].as_str().unwrap(),
                    tool["description"].as_str().unwrap()
                )
            })
            .collect();
        self.0.lock().unwrap().push(offered);
        reply(Vec::new())
    }
}

/// Calls `named` once, then answers.
struct Calls(&'static str);

impl Model for Calls {
    async fn generate(&self, thread: &Thread, _tools: &[Value]) -> ModelResponse {
        let answered = thread
            .messages()
            .iter()
            .any(|message| matches!(message, Message::ToolResults(_)));
        if answered {
            return reply(Vec::new());
        }
        reply(vec![ToolCall {
            id: "call-1".to_owned(),
            name: self.0.to_owned(),
            args: json!({}),
        }])
    }
}

fn reply(tool_calls: Vec<ToolCall>) -> ModelResponse {
    let stop = if tool_calls.is_empty() {
        StopReason::EndTurn
    } else {
        StopReason::ToolUse
    };
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: "done".to_owned(),
        tool_calls,
        stop,
    }
}

struct Named {
    name: &'static str,
    description: &'static str,
    called: AtomicBool,
}

fn tool(name: &'static str, description: &'static str) -> Arc<Named> {
    Arc::new(Named {
        name,
        description,
        called: AtomicBool::new(false),
    })
}

impl Tool for Named {
    fn name(&self) -> &str {
        self.name
    }

    fn definition(&self) -> Value {
        json!({"name": self.name, "description": self.description, "input_schema": {"type": "object"}})
    }

    fn call<'a>(
        &'a self,
        _request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        self.called.store(true, Ordering::Release);
        Box::pin(async { Ok(String::new().into()) })
    }
}

fn registry(tools: &[&Arc<Named>]) -> Registry {
    Registry::new(
        tools
            .iter()
            .map(|tool| -> Arc<dyn Tool> {
                let cloned: Arc<Named> = Arc::clone(tool);
                cloned
            })
            .collect(),
    )
}

fn said(request: &str) -> Prompt {
    let address = Address::new("test", "C1", "thread").unwrap();
    Prompt::new(address, request, "system", "héllo ☕").unwrap()
}

fn offered(tools: &[&str]) -> Vec<String> {
    tools.iter().map(ToString::to_string).collect()
}

#[tokio::test]
async fn the_next_message_on_a_lane_offers_the_tools_of_the_registry_that_runs_it() {
    let runtime = Runtime::new(MemoryStore::default());
    let (search, create) = (tool("search", "v1"), tool("create", "v1"));
    let model = ToolNames::default();
    runtime
        .accept_with_tools(&said("m1"), &registry(&[&search]))
        .await
        .unwrap();
    runtime
        .run_next(&model, &registry(&[&search]))
        .await
        .unwrap();

    runtime.accept(&said("m2")).await.unwrap();
    runtime
        .run_next(&model, &registry(&[&search, &create]))
        .await
        .unwrap();
    assert_eq!(model.last(), offered(&["search:v1", "create:v1"]));

    runtime.accept(&said("m3")).await.unwrap();
    runtime
        .run_next(&model, &registry(&[&create]))
        .await
        .unwrap();
    assert_eq!(model.last(), offered(&["create:v1"]));
}

async fn spawn(store: &MemoryStore, grant: Vec<Value>) -> Runtime<MemoryStore> {
    let runtime = Runtime::new(store.clone());
    let parent = runtime.accept(&said("parent")).await.unwrap();
    store
        .claim("parent", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let config = SubagentConfig::new("reviewer", "review", ExecutionProfile::default())
        .unwrap()
        .with_tools(grant);
    let request = SubagentRequest::new(parent.operation.id, "spawn", "task", config).unwrap();
    runtime.spawn_subagent(&request).await.unwrap();
    runtime
}

#[tokio::test]
async fn a_subagent_keeps_its_spawn_grant_and_sees_current_schemas() {
    let (old, new) = (tool("search", "v1"), tool("search", "v2"));
    let runtime = spawn(&MemoryStore::default(), vec![old.definition()]).await;
    let model = ToolNames::default();

    runtime
        .run_next(&model, &registry(&[&new, &tool("create", "v1")]))
        .await
        .unwrap();

    let seen = model.0.lock().unwrap().clone();
    assert_eq!(seen, vec![offered(&["search:v2"])]);
}

#[tokio::test]
async fn a_subagent_spawned_without_tools_cannot_call_any() {
    let runtime = spawn(&MemoryStore::default(), Vec::new()).await;
    let create = tool("create", "v1");

    runtime
        .run_next(&Calls("create"), &registry(&[&create]))
        .await
        .unwrap();

    assert!(!create.called.load(Ordering::Acquire));
}

/// A source whose next listing adds `create`.
struct Growing(Mutex<Vec<Arc<dyn Tool>>>);

impl agent::ToolSource for Growing {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.0.lock().unwrap().clone()
    }

    fn refresh(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.0.lock().unwrap().push(tool("create", "v1"));
        Box::pin(async {})
    }
}

#[tokio::test]
async fn a_claim_lists_tool_sources_again_before_offering_tools() {
    let runtime = Runtime::new(MemoryStore::default());
    let source = Arc::new(Growing(Mutex::new(vec![tool("search", "v1")])));
    let tools = Registry::from_sources(vec![source]);
    let model = ToolNames::default();
    runtime
        .accept_with_tools(&said("m1"), &tools)
        .await
        .unwrap();

    runtime.run_next(&model, &tools).await.unwrap();

    assert_eq!(model.last(), offered(&["search:v1", "create:v1"]));
}
