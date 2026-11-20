//! Accepted tool permissions and completion checks apply at execution boundaries.
#![allow(clippy::too_many_lines)] // Table-driven workflow scenarios.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "Scripted model performs no I/O."
)]
use agent::{
    ExecutionProfile, Message, Model, ModelResponse, Outcome, Registry, StopReason, Thread,
    ToolCall, ToolContract, ToolError, ToolInvocation, typed_tool,
};
use brainstone_runtime::{Address, MemoryStore, Prompt, Runtime};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Script {
    call: bool,
    expected_tools: usize,
}
impl Model for Script {
    async fn generate(&self, thread: &Thread, tools: &[Value]) -> ModelResponse {
        assert_eq!(tools.len(), self.expected_tools);
        let mut answer = response("done");
        if self.call && !matches!(thread.last(), Message::ToolResults(_)) {
            answer.tool_calls = vec![ToolCall {
                id: "one".into(),
                name: "Write".into(),
                args: json!({}),
            }];
            answer.stop = StopReason::ToolUse;
        }
        answer
    }
}
fn response(text: &str) -> ModelResponse {
    ModelResponse {
        text: text.into(),
        reasoning: vec![],
        tool_calls: vec![],
        stop: StopReason::EndTurn,
        model: None,
    }
}
fn tools(count: Arc<AtomicUsize>) -> Registry {
    let tool = typed_tool(
        "Write",
        "Record a value",
        move |_: serde_json::Map<String, Value>, _: ToolInvocation| {
            let count = Arc::clone(&count);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok::<_, ToolError>(json!({"written":true}))
            }
        },
    )
    .unwrap();
    Registry::new(vec![Arc::new(tool)])
}
fn prompt(contract: ToolContract) -> Prompt {
    Prompt::new(
        Address::new("test", "contract", "main").unwrap(),
        "one",
        "system",
        "go",
    )
    .unwrap()
    .with_profile(ExecutionProfile::default().with_tool_contract(contract))
}
#[tokio::test]
async fn accepted_allowlist_restricts_advertising_and_hallucinated_dispatch() {
    let count = Arc::new(AtomicUsize::new(0));
    let runtime = Runtime::new(MemoryStore::default());
    runtime
        .accept(&prompt(ToolContract::only(Vec::<String>::new()).unwrap()))
        .await
        .unwrap();
    let operation = runtime
        .run_next(
            &Script {
                call: true,
                expected_tools: 0,
            },
            &tools(Arc::clone(&count)),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(operation.outcome, Some(Outcome::Completed { .. })));
    assert_eq!(count.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn prose_cannot_satisfy_a_required_effect() {
    for call in [false, true] {
        let count = Arc::new(AtomicUsize::new(0));
        let contract = ToolContract::only(["Write"])
            .unwrap()
            .require_success(["Write"])
            .unwrap();
        let runtime = Runtime::new(MemoryStore::default());
        runtime.accept(&prompt(contract)).await.unwrap();
        let operation = runtime
            .run_next(
                &Script {
                    call,
                    expected_tools: 1,
                },
                &tools(Arc::clone(&count)),
            )
            .await
            .unwrap()
            .unwrap();
        match (call, operation.outcome.unwrap()) {
            (true, Outcome::Completed { text }) => assert_eq!(text, "done"),
            (false, Outcome::Failed { code, .. }) => assert_eq!(code, "tool_requirement_unmet"),
            other => panic!("unexpected result: {other:?}"),
        }
        assert_eq!(count.load(Ordering::SeqCst), usize::from(call));
    }
}

struct NoRepair;
impl Model for NoRepair {
    async fn generate(&self, _: &Thread, _: &[Value]) -> ModelResponse {
        response("I did it")
    }
    async fn output_feedback(&self, _: &Thread) -> Option<String> {
        panic!("missing effects must fail without repair inference")
    }
}
#[tokio::test]
async fn missing_effect_does_not_invoke_output_repair() {
    let runtime = Runtime::new(MemoryStore::default());
    let contract = ToolContract::only(["Write"])
        .unwrap()
        .require_success(["Write"])
        .unwrap();
    runtime.accept(&prompt(contract)).await.unwrap();
    let operation = runtime
        .run_next(&NoRepair, &tools(Arc::new(AtomicUsize::new(0))))
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(operation.outcome, Some(Outcome::Failed { code, .. }) if code == "tool_requirement_unmet")
    );
}
#[tokio::test]
async fn unavailable_contract_tools_fail_before_inference() {
    struct NoModel;
    impl Model for NoModel {
        async fn generate(&self, _: &Thread, _: &[Value]) -> ModelResponse {
            panic!("unavailable contract must fail before inference")
        }
    }
    let runtime = Runtime::new(MemoryStore::default());
    runtime
        .accept(&prompt(ToolContract::only(["Missing"]).unwrap()))
        .await
        .unwrap();
    let operation = runtime
        .run_next(&NoModel, &Registry::default())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(operation.outcome, Some(Outcome::Failed { code, .. }) if code == "tool_contract_unavailable")
    );
}
