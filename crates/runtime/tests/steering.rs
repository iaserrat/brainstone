#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Steering interrupts inference and fences unstarted actions at durable boundaries.
#![allow(clippy::too_many_lines)] // End-to-end concurrency and replay scenarios.
use agent::{
    Event, Message, Model, ModelResponse, Outcome, Reasoning, Registry, Sink, StopReason, Thread,
    Tool, ToolCall, ToolError, ToolOutput, ToolRequest,
};
use brainstone_runtime::{Address, Admitted, DurableStore, MemoryStore, Prompt, Runtime, Store};
use serde_json::{Value, json};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::Notify;
#[path = "support/database.rs"]
mod database;

#[tokio::test]
async fn memory_steering_interrupts_inference_and_unstarted_actions() {
    check(MemoryStore::default()).await;
}

#[tokio::test]
async fn sqlite_steering_interrupts_inference_and_unstarted_actions() {
    let database = database::Database::sqlite();
    let store = DurableStore::connect(&database.url).await.unwrap();
    check(store.clone()).await;
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_steering_interrupts_inference_and_unstarted_actions() {
    let database = database::Database::postgres().await;
    let store = DurableStore::connect(&database.url).await.unwrap();
    check(store.clone()).await;
    store.close().await;
    database.close().await;
}

async fn check(store: impl Store + 'static) {
    interrupt_reasoning(store.clone()).await;
    interrupt_batch(store).await;
}

fn prompt(lane: &str, id: &str, text: &str) -> Prompt {
    Prompt::new(
        Address::new("steering", "session", lane).unwrap(),
        id,
        "system",
        text,
    )
    .unwrap()
}

fn response(text: &str) -> ModelResponse {
    ModelResponse {
        model: None,
        text: text.into(),
        reasoning: vec![],
        tool_calls: vec![],
        stop: StopReason::EndTurn,
    }
}

async fn interrupt_reasoning(store: impl Store + 'static) {
    let runtime = Runtime::new(store);
    runtime
        .accept(&prompt("reasoning", "original", "old instruction"))
        .await
        .unwrap();
    let model = Arc::new(Thinking {
        entered: Notify::new(),
        calls: AtomicUsize::new(0),
    });
    let running = tokio::spawn({
        let (runtime, model) = (runtime.clone(), model.clone());
        async move { runtime.run_next(model.as_ref(), &Registry::default()).await }
    });
    model.entered.notified().await;
    let correction = prompt("reasoning", "correction", "corrected ☕");
    assert!(matches!(
        runtime
            .admit_with_tools(&correction, &Registry::default())
            .await
            .unwrap(),
        Admitted::Steering {
            duplicate: false,
            ..
        }
    ));
    let result = tokio::time::timeout(Duration::from_secs(2), running).await;
    assert!(
        result.is_ok(),
        "steering did not interrupt the in-flight reasoning request"
    );
    let operation = result.unwrap().unwrap().unwrap().unwrap();
    assert_eq!(
        operation.outcome,
        Some(Outcome::Completed {
            text: "corrected ☕".into()
        })
    );
}

struct Thinking {
    entered: Notify,
    calls: AtomicUsize,
}
impl Model for Thinking {
    async fn generate(&self, _: &Thread, _: &[Value]) -> ModelResponse {
        unreachable!("streaming")
    }
    async fn generate_stream<S: Sink + Send>(
        &self,
        thread: &Thread,
        _: &[Value],
        sink: &mut S,
    ) -> Result<ModelResponse, S::Error> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            sink.emit(Event::ModelDelta {
                delta: "provisional old answer".into(),
            })
            .await?;
            self.entered.notify_one();
            return std::future::pending().await;
        }
        assert_eq!(
            thread.messages(),
            &[
                Message::User("old instruction".into()),
                Message::User("corrected ☕".into())
            ]
        );
        Ok(response("corrected ☕"))
    }
}

async fn interrupt_batch(store: impl Store + 'static) {
    let runtime = Runtime::new(store);
    runtime
        .accept(&prompt("batch", "original", "run two actions"))
        .await
        .unwrap();
    let tool = Arc::new(Action {
        entered: Notify::new(),
        release: Notify::new(),
        calls: AtomicUsize::new(0),
    });
    let registry = Registry::new(vec![tool.clone()]);
    let model = Arc::new(Batch {
        calls: AtomicUsize::new(0),
        captured: Mutex::new(vec![]),
    });
    let running = tokio::spawn({
        let (runtime, model) = (runtime.clone(), model.clone());
        async move { runtime.run_next(model.as_ref(), &registry).await }
    });
    tool.entered.notified().await;
    runtime
        .admit_with_tools(
            &prompt("batch", "stop", "stop remaining actions"),
            &Registry::default(),
        )
        .await
        .unwrap();
    tool.release.notify_one();
    running.await.unwrap().unwrap().unwrap();
    assert_eq!(
        tool.calls.load(Ordering::SeqCst),
        1,
        "unstarted action ran after steering"
    );
    let messages = model.captured.lock().unwrap();
    let Message::Assistant(assistant) = &messages[1] else {
        panic!("reasoning response")
    };
    assert_eq!(assistant.reasoning, reasoning());
    let Message::ToolResults(results) = &messages[2] else {
        panic!("paired results before steering")
    };
    assert_eq!(results.len(), 2);
    assert!(!results[0].is_error);
    assert!(results[1].is_error);
    assert_eq!(
        serde_json::from_str::<Value>(&results[1].content).unwrap()["error"],
        "tool_call_superseded"
    );
    assert_eq!(messages[3], Message::User("stop remaining actions".into()));
}

fn reasoning() -> Vec<Reasoning> {
    vec![Reasoning::Opaque {
        provider: "openai-chat-completions".into(),
        data: json!({"reasoning_details":[{"type":"reasoning.encrypted","data":"unaltered","signature":"signature"}]}),
    }]
}

struct Batch {
    calls: AtomicUsize,
    captured: Mutex<Vec<Message>>,
}
impl Model for Batch {
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return ModelResponse {
                reasoning: reasoning(),
                tool_calls: ["one", "two"]
                    .into_iter()
                    .map(|id| ToolCall {
                        id: id.into(),
                        name: "Action".into(),
                        args: json!({}),
                    })
                    .collect(),
                stop: StopReason::ToolUse,
                ..response("")
            };
        }
        *self.captured.lock().unwrap() = thread.messages().to_vec();
        response("stopped")
    }
}

struct Action {
    entered: Notify,
    release: Notify,
    calls: AtomicUsize,
}
impl Tool for Action {
    fn name(&self) -> &'static str {
        "Action"
    }
    fn definition(&self) -> Value {
        json!({"name":"Action","description":"External action","input_schema":{"type":"object"}})
    }
    fn call<'a>(
        &'a self,
        _: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Ok(ToolOutput::from("committed".to_owned()))
        })
    }
}
