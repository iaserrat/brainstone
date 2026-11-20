#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Required authorization fails closed before tools reach the dispatch boundary.
#![allow(clippy::too_many_lines)] // Table-driven policy scenarios.
use agent::{Message, Model, ModelResponse, Registry, StopReason, Thread, ToolCall};
use brainstone_runtime::{Address, Config, MemoryStore, Prompt, Runtime, ToolPolicy};
use serde_json::{Value, json};
use std::time::Duration;
struct Attempt;
impl Model for Attempt {
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        match thread.last() {
            Message::ToolResults(results) => {
                assert!(results[0].is_error);
                answer(&results[0].content)
            }
            _ => ModelResponse {
                tool_calls: vec![ToolCall {
                    id: "one".into(),
                    name: "External".into(),
                    args: json!({}),
                }],
                stop: StopReason::ToolUse,
                ..answer("")
            },
        }
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
#[tokio::test]
async fn unsupported_stores_and_denied_tools_return_stable_failures() {
    for (require, deny, expected) in [
        (vec!["*".into()], vec![], "tool_approval_unsupported"),
        (
            vec!["*".into()],
            vec!["External".into()],
            "tool_policy_denied",
        ),
    ] {
        let policy = ToolPolicy::new(require, deny, "executor".into()).unwrap();
        let config = Config::new("worker", Duration::from_secs(30))
            .unwrap()
            .with_tool_policy(policy);
        let runtime = Runtime::with_config(MemoryStore::default(), config);
        let prompt = Prompt::new(
            Address::new("policy", "session", "lane").unwrap(),
            "request",
            "system",
            "go",
        )
        .unwrap();
        runtime.accept(&prompt).await.unwrap();
        let operation = runtime
            .run_next(&Attempt, &Registry::default())
            .await
            .unwrap()
            .unwrap();
        let Some(agent::Outcome::Completed { text }) = operation.outcome else {
            panic!("outcome")
        };
        assert_eq!(
            serde_json::from_str::<Value>(&text).unwrap()["error"],
            expected
        );
    }
}
#[test]
fn invalid_policy_names_fail_at_configuration() {
    for name in ["", "mcp__*", "has space", "💥"] {
        assert_eq!(
            ToolPolicy::new(vec![name.into()], vec![], "executor".into())
                .unwrap_err()
                .code(),
            "invalid_input"
        );
    }
}
