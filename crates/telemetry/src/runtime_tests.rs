#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
// Scenarios assert real runtime effects and interruption, rather than span helpers.
#![allow(clippy::too_many_lines)]
use agent::{Model, ModelResponse, Registry, StopReason, Thread};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use runtime::{Address, MemoryStore, Prompt, Runtime};
use tracing_subscriber::prelude::*;

struct Answer(bool);
impl Model for Answer {
    async fn generate(&self, _: &Thread, _: &[serde_json::Value]) -> ModelResponse {
        ModelResponse {
            reasoning: vec![],
            text: "PATIENT-COMPLETION".into(),
            tool_calls: vec![],
            model: None,
            stop: if self.0 {
                StopReason::EndTurn
            } else {
                StopReason::Error {
                    code: "provider_unavailable".into(),
                    message: "PATIENT-ERROR".into(),
                }
            },
        }
    }
}

#[tokio::test]
async fn operation_outcome_follows_durable_settlement_and_model_semantics() {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
    let _guard = tracing::subscriber::set_default(subscriber);
    for success in [true, false] {
        let runtime = Runtime::new(MemoryStore::default());
        runtime.accept(&prompt()).await.unwrap();
        runtime
            .run_next(&Answer(success), &Registry::default())
            .await
            .unwrap()
            .unwrap();
    }
    let spans = exporter.get_finished_spans().unwrap();
    for kind in ["operation", "model"] {
        let observed: Vec<_> = spans
            .iter()
            .filter(|span| attr(span, "kind") == kind)
            .collect();
        assert_eq!(observed.len(), 2, "missing {kind}");
        assert_eq!(attr(observed[0], "outcome"), "ok");
        assert_eq!(attr(observed[1], "outcome"), "failed");
    }
    assert!(!format!("{spans:?}").contains("PATIENT"));
}

fn prompt() -> Prompt {
    Prompt::new(
        Address::new("test", "channel", "thread").unwrap(),
        "event",
        "PATIENT-SYSTEM",
        "PATIENT-PROMPT",
    )
    .unwrap()
}
fn attr(span: &opentelemetry_sdk::trace::SpanData, name: &str) -> String {
    span.attributes
        .iter()
        .find(|value| value.key.as_str() == name)
        .map_or(String::new(), |value| value.value.to_string())
}

#[tokio::test]
async fn abandoned_attempt_reopens_records_and_resumes_without_repeating_tool_work() {
    use std::time::Duration;
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
    let _guard = tracing::subscriber::set_default(subscriber);
    let store = MemoryStore::default();
    let config = runtime::Config::new("before", Duration::from_millis(30)).unwrap();
    let runtime = Runtime::with_config(store.clone(), config);
    let accepted = runtime.accept(&prompt()).await.unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_millis(5),
            runtime.run_next(&Interrupted, &Registry::default())
        )
        .await
        .is_err()
    );
    tokio::time::sleep(Duration::from_millis(40)).await;
    let reopened = Runtime::new(store);
    let operation = reopened
        .run_next(&Answer(true), &Registry::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(operation.id, accepted.operation.id);
    assert_eq!(operation.state, runtime::OperationState::Completed);
    let spans = exporter.get_finished_spans().unwrap();
    let attempts: Vec<_> = spans
        .iter()
        .filter(|span| attr(span, "kind") == "operation")
        .collect();
    assert_eq!(attempts.len(), 2);
    assert_ne!(attr(attempts[0], "attempt"), attr(attempts[1], "attempt"));
    assert_ne!(
        attempts[0].span_context.trace_id(),
        attempts[1].span_context.trace_id()
    );
    assert_eq!(
        attr(attempts[0], "operation_id"),
        attr(attempts[1], "operation_id")
    );
    assert_eq!(attr(attempts[1], "outcome"), "ok");
    assert_eq!(
        spans
            .iter()
            .filter(|span| attr(span, "kind") == "tool")
            .count(),
        1
    );
    assert_eq!(
        spans
            .iter()
            .filter(|span| attr(span, "kind") == "recovery")
            .count(),
        2
    );
}
struct Interrupted;
impl Model for Interrupted {
    async fn generate(&self, thread: &Thread, _: &[serde_json::Value]) -> ModelResponse {
        if matches!(thread.last(), agent::Message::ToolResults(_)) {
            return std::future::pending().await;
        }
        let mut response = Answer(true).generate(thread, &[]).await;
        response.stop = StopReason::ToolUse;
        response.tool_calls.push(agent::ToolCall {
            id: "PATIENT-ID".into(),
            name: "PATIENT-TOOL".into(),
            args: serde_json::json!({"patient":"PATIENT-ARG"}),
        });
        response
    }
}

#[tokio::test]
async fn explicit_child_cancellation_is_observed_after_lease_reconciliation() {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
    let _guard = tracing::subscriber::set_default(subscriber);
    let runtime = Runtime::new(MemoryStore::default());
    let parent = runtime.accept(&prompt()).await.unwrap().operation.id;
    let config = runtime::SubagentConfig::new(
        "PATIENT-NAME",
        "PATIENT-SYSTEM",
        agent::ExecutionProfile::default(),
    )
    .unwrap();
    let request =
        runtime::SubagentRequest::new(parent, "PATIENT-KEY", "PATIENT-TASK", config).unwrap();
    let child = runtime.spawn_subagent(&request).await.unwrap().agent;
    let model = Cancel {
        runtime: runtime.clone(),
        parent,
        child: child.id,
    };
    let result = runtime
        .run_operation(child.operation.id, &model, &Registry::default())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(result.outcome, Some(agent::Outcome::Failed { ref code, .. }) if code == "agent_cancelled")
    );
    let spans = exporter.get_finished_spans().unwrap();
    let operation = spans
        .iter()
        .find(|span| attr(span, "kind") == "operation")
        .unwrap();
    assert_eq!(attr(operation, "outcome"), "cancelled");
    let spawn = spans
        .iter()
        .find(|span| attr(span, "kind") == "subagent")
        .unwrap();
    assert_eq!(attr(spawn, "parent_operation_id"), parent.get().to_string());
    assert_eq!(
        attr(spawn, "child_operation_id"),
        child.operation.id.get().to_string()
    );
    assert!(!format!("{spans:?}").contains("PATIENT"));
}
struct Cancel {
    runtime: Runtime<MemoryStore>,
    parent: runtime::OperationId,
    child: runtime::SubagentId,
}
impl Model for Cancel {
    async fn generate(&self, thread: &Thread, _: &[serde_json::Value]) -> ModelResponse {
        self.runtime
            .stop_subagent(self.parent, self.child)
            .await
            .unwrap();
        Answer(true).generate(thread, &[]).await
    }
}
