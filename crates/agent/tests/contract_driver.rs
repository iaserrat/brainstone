//! The non-durable driver cannot bypass accepted permissions or approvals.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "Scripted models and sinks perform no I/O."
)]
use brainstone_agent::{
    Event, ExecutionProfile, Model, ModelResponse, Outcome, Registry, Sink, StopReason, Thread,
    ToolContract, ToolError, ToolInvocation, run, typed_tool,
};
use serde_json::{Value, json};
use std::{convert::Infallible, sync::Arc};
struct Events;
impl Sink for Events {
    type Error = Infallible;
    async fn emit(&mut self, _: Event) -> Result<(), Infallible> {
        Ok(())
    }
}
struct NoTools;
impl Model for NoTools {
    async fn generate(&self, _: &Thread, tools: &[Value]) -> ModelResponse {
        let expected: &[Value] = &[];
        assert_eq!(tools, expected);
        ModelResponse {
            text: "done".into(),
            tool_calls: vec![],
            reasoning: vec![],
            stop: StopReason::EndTurn,
            model: None,
        }
    }
}
fn tools() -> Registry {
    Registry::new(vec![Arc::new(
        typed_tool(
            "Write",
            "Write",
            |_: serde_json::Map<String, Value>, _: ToolInvocation| async {
                Ok::<_, ToolError>(json!({}))
            },
        )
        .unwrap(),
    )])
}
#[tokio::test]
async fn standalone_driver_filters_the_accepted_allowlist() {
    let profile = ExecutionProfile::default()
        .with_tool_contract(ToolContract::only(Vec::<String>::new()).unwrap());
    let mut thread = Thread::new_with_profile("system", "hello", profile);
    assert!(matches!(
        run(&mut thread, &NoTools, &tools(), &mut Events)
            .await
            .unwrap(),
        Outcome::Completed { .. }
    ));
}
#[tokio::test]
async fn standalone_driver_cannot_execute_an_approval_contract() {
    struct NoInference;
    impl Model for NoInference {
        async fn generate(&self, _: &Thread, _: &[Value]) -> ModelResponse {
            panic!("approval needs durable authorization")
        }
    }
    let contract = ToolContract::only(["Write"])
        .unwrap()
        .require_approval(["Write"])
        .unwrap();
    let mut thread = Thread::new_with_profile(
        "system",
        "hello",
        ExecutionProfile::default().with_tool_contract(contract),
    );
    assert!(
        matches!(run(&mut thread, &NoInference, &tools(), &mut Events).await.unwrap(), Outcome::Failed { code, .. } if code == "tool_approval_unsupported")
    );
}
