#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! `PostgreSQL` crash-recovery cases include their complete journal boundaries.
#![allow(clippy::too_many_lines)] // Table-driven scenarios; setup helpers remain bounded.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use agent::{Event, ExecutionProfile, Message, Model, ModelResponse, Registry, StopReason, Thread};
use brainstone_runtime::{Address, PostgresStore, Prompt, Runtime, Store};

const TTL: Duration = Duration::from_secs(1);
const RESUME_WAIT: Duration = Duration::from_millis(1100);

#[path = "support/subagent_retries.rs"]
mod subagent_retries;

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn followup_retries_preserve_the_current_child_task() {
    subagent_retries::check(&store().await, &prompt("followup-retries", "parent")).await;
}

struct Calling(AtomicUsize);

impl Model for Calling {
    async fn generate(&self, _: &Thread, _: &[serde_json::Value]) -> ModelResponse {
        let index = self.0.fetch_add(1, Ordering::SeqCst);
        ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: String::new(),
            tool_calls: vec![agent::ToolCall {
                id: format!("call-{index}"),
                name: "missing".to_owned(),
                args: serde_json::json!({}),
            }],
            stop: StopReason::ToolUse,
        }
    }
}

async fn store() -> PostgresStore {
    PostgresStore::connect(&std::env::var("BRAINSTONE_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap()
}

fn prompt(lane: &str, request: &str) -> Prompt {
    let conversation = format!("{lane}-{}", std::process::id());
    let address = Address::new("hot-review", &conversation, "main").unwrap();
    Prompt::new(address, request, "system", "start")
        .unwrap()
        .with_profile(ExecutionProfile::new(None, None, 3, 0).unwrap())
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn steering_after_compaction_restores_the_full_user_turn_budget() {
    for lane in ["uncompacted-steering", "compacted-steering"] {
        let store = store().await;
        let first = prompt(lane, "first");
        let id = store.accept(&first).await.unwrap().operation.id;
        let claim = store
            .claim_operation(id, "before-crash", TTL)
            .await
            .unwrap()
            .unwrap();
        let model = Calling(AtomicUsize::new(0));
        let response = model.generate(&claim.thread, &[]).await;
        let call = response.tool_calls[0].clone();
        let tools = Registry::default();
        let result = agent::execute_tool(&response.tool_calls[0], &tools, None).await;
        for event in [
            Event::RunStarted,
            Event::Appended {
                message: Message::Assistant(response),
            },
            Event::ToolStarted {
                id: call.id,
                name: call.name,
                args: call.args,
            },
            Event::ToolFinished {
                result: result.clone(),
            },
            Event::Appended {
                message: Message::ToolResults(vec![result]),
            },
            Event::Compacted {
                summary: "brief".to_owned(),
                steps: 1,
            },
        ] {
            if lane == "uncompacted-steering" && matches!(event, Event::Compacted { .. }) {
                continue;
            }
            store.record(&claim.lease, &event).await.unwrap();
        }
        store
            .admit_with_tools(&prompt(lane, "steer"), &[])
            .await
            .unwrap();
        assert_eq!(store.steering(&claim.lease).await.unwrap().len(), 1);
        store.close().await;
        tokio::time::sleep(RESUME_WAIT).await;
        let reopened = self::store().await;
        let model = Calling(AtomicUsize::new(0));
        let operation = Runtime::new(reopened)
            .run_operation(id, &model, &tools)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(operation.outcome,
            Some(agent::Outcome::Failed { ref code, .. }) if code == "step_budget_exhausted"));
        assert_eq!(
            model.0.load(Ordering::SeqCst),
            3,
            "a new user turn must receive all three turns"
        );
    }
}
