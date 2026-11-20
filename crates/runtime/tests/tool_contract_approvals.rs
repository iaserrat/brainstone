//! Accepted tool contracts cannot bypass durable approval or widen dispatch.
#![allow(clippy::too_many_lines)] // Scenario tables keep authorization assertions together.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "Scripted model resolves on poll."
)]

use agent::{
    ExecutionProfile, Message, Model, ModelResponse, Outcome, Registry, StopReason, Thread, Tool,
    ToolCall, ToolContract, ToolError, ToolOutput, ToolRequest,
};
use brainstone_runtime::{Address, Config, DurableStore, MemoryStore, Prompt, Runtime, ToolPolicy};
use serde_json::{Value, json};
use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const WAIT: Duration = Duration::from_secs(3);

struct Attempt {
    name: &'static str,
    error: Option<&'static str>,
}
impl Model for Attempt {
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        if let Message::ToolResults(results) = thread.last() {
            assert_eq!(results.len(), 1);
            let value: Value = serde_json::from_str(&results[0].content).unwrap();
            assert_eq!(results[0].is_error, self.error.is_some());
            if let Some(error) = self.error {
                assert_eq!(value["error"], error);
            }
            return answer("The write completed.");
        }
        let mut response = answer("");
        response.tool_calls = vec![ToolCall {
            id: "call-1".into(),
            name: self.name.into(),
            args: json!({"value":"Zoë"}),
        }];
        response.stop = StopReason::ToolUse;
        response
    }
}

struct Write(Arc<AtomicUsize>);
impl Tool for Write {
    fn name(&self) -> &'static str {
        "Write"
    }
    fn definition(&self) -> Value {
        json!({"name":"Write","description":"Commit a value","input_schema":{"type":"object"}})
    }
    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            assert!(request.idempotency_key.is_some());
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(json!({"outcome":"success"}).to_string().into())
        })
    }
}

fn answer(text: &str) -> ModelResponse {
    ModelResponse {
        model: None,
        text: text.into(),
        reasoning: vec![],
        tool_calls: vec![],
        stop: StopReason::EndTurn,
    }
}
fn tools() -> (Registry, Arc<AtomicUsize>) {
    let effects = Arc::new(AtomicUsize::new(0));
    (
        Registry::new(vec![Arc::new(Write(Arc::clone(&effects)))]),
        effects,
    )
}
fn prompt() -> Prompt {
    let contract = ToolContract::only(["Write"])
        .unwrap()
        .require_approval(["Write"])
        .unwrap()
        .require_success(["Write"])
        .unwrap();
    Prompt::new(
        Address::new("approval", "session", "lane").unwrap(),
        "request",
        "system",
        "write",
    )
    .unwrap()
    .with_profile(ExecutionProfile::default().with_tool_contract(contract))
}
fn failed(outcome: Option<Outcome>) {
    let Some(Outcome::Failed { code, .. }) = outcome else {
        panic!("a claim of success cannot satisfy an unexecuted tool");
    };
    assert_eq!(code, "tool_requirement_unmet");
}
async fn pending(store: &DurableStore) -> String {
    tokio::time::timeout(WAIT, async {
        loop {
            if let Some((id, _)) = store.pending_approvals().await.unwrap().first() {
                return id.clone();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("accepted approval must become pending")
}
async fn sqlite() -> (tempfile::TempDir, DurableStore) {
    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", directory.path().join("runtime.db").display());
    let store = DurableStore::connect(&url).await.unwrap();
    (directory, store)
}

#[tokio::test]
async fn accepted_approval_waits_for_an_exact_durable_decision_without_live_policy() {
    for approve in [true, false] {
        let (_directory, store) = sqlite().await;
        let runtime = Runtime::new(store.clone());
        let (tools, effects) = tools();
        let accepted = runtime.accept_with_tools(&prompt(), &tools).await.unwrap();
        let run = tokio::spawn(async move {
            runtime
                .run_next(
                    &Attempt {
                        name: "Write",
                        error: (!approve).then_some("tool_approval_denied"),
                    },
                    &tools,
                )
                .await
        });
        let id = pending(&store).await;
        assert_eq!(
            effects.load(Ordering::SeqCst),
            0,
            "pending approval cannot dispatch"
        );
        assert!(!run.is_finished());
        let record = store.approval(&id).await.unwrap().unwrap();
        assert_eq!(record.request.operation, accepted.operation.id);
        assert_eq!(record.request.call.args, json!({"value":"Zoë"}));
        assert_eq!(record.request.call.name, "Write");
        assert_eq!(record.request.definition["name"], "Write");
        assert!(
            record
                .request
                .call_key
                .starts_with(&format!("operation/{}/", accepted.operation.id.get()))
        );
        assert_ne!(record.request.binding, "");
        store
            .decide_approval(&id, approve, "operator")
            .await
            .unwrap();
        let operation = tokio::time::timeout(WAIT, run)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(effects.load(Ordering::SeqCst), usize::from(approve));
        if approve {
            assert!(matches!(operation.outcome, Some(Outcome::Completed { .. })));
        } else {
            failed(operation.outcome);
        }
        store.close().await;
    }
}

#[tokio::test]
async fn accepted_approval_fails_closed_on_a_store_without_authorization_support() {
    let runtime = Runtime::new(MemoryStore::default());
    let (tools, effects) = tools();
    runtime.accept_with_tools(&prompt(), &tools).await.unwrap();
    let operation = runtime
        .run_next(
            &Attempt {
                name: "Write",
                error: Some("tool_approval_unsupported"),
            },
            &tools,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    failed(operation.outcome);
}

#[tokio::test]
async fn live_operator_denial_overrides_the_accepted_contract() {
    let policy = ToolPolicy::new(vec![], vec!["Write".into()], "executor-v1".into()).unwrap();
    let config = Config::new("worker", Duration::from_secs(30))
        .unwrap()
        .with_tool_policy(policy);
    let runtime = Runtime::with_config(MemoryStore::default(), config);
    let (tools, effects) = tools();
    runtime.accept_with_tools(&prompt(), &tools).await.unwrap();
    let operation = runtime
        .run_next(
            &Attempt {
                name: "Write",
                error: Some("tool_policy_denied"),
            },
            &tools,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    failed(operation.outcome);
}

#[tokio::test]
async fn a_forbidden_hallucinated_call_never_creates_an_approval_request() {
    let (_directory, store) = sqlite().await;
    let policy = ToolPolicy::new(vec!["*".into()], vec![], "executor-v1".into()).unwrap();
    let config = Config::new("worker", Duration::from_secs(30))
        .unwrap()
        .with_tool_policy(policy);
    let runtime = Runtime::with_config(store.clone(), config);
    let (tools, effects) = tools();
    runtime.accept_with_tools(&prompt(), &tools).await.unwrap();
    let operation = tokio::time::timeout(
        WAIT,
        runtime.run_next(
            &Attempt {
                name: "Forbidden",
                error: Some("tool_contract_denied"),
            },
            &tools,
        ),
    )
    .await
    .expect("a forbidden call must not park for approval")
    .unwrap()
    .unwrap();
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    assert_eq!(
        store.pending_approvals().await.unwrap(),
        Vec::<(String, i64)>::new()
    );
    failed(operation.outcome);
    store.close().await;
}
