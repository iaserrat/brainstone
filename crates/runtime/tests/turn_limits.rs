//! Accepted limits survive a crash, compaction, and a changed deployment default.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test models run when polled."
)]
#![allow(clippy::too_many_lines)] // Scenario tables keep each recovery boundary visible.

use std::time::Duration;

use agent::{
    Event, ExecutionProfile, Message, Model, ModelResponse, Outcome, Registry, StopReason, Thread,
};
use brainstone_runtime::{Address, Admitted, DurableStore, Prompt, Runtime, Store};

#[path = "support/database.rs"]
mod database;

const RESPONSES: usize = agent::MAX_STEPS + 1;

#[tokio::test]
async fn sqlite_recovers_unlimited_and_explicitly_limited_operations() {
    let database = database::Database::sqlite();
    check(&database.url).await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_recovers_unlimited_and_explicitly_limited_operations() {
    let database = database::Database::postgres().await;
    check(&database.url).await;
    database.close().await;
}

async fn check(url: &str) {
    for limit in [None, Some(RESPONSES)] {
        recover(url, limit).await;
    }
}

async fn recover(url: &str, limit: Option<usize>) {
    let store = DurableStore::connect(url).await.unwrap();
    let lane = if limit.is_some() {
        "limited"
    } else {
        "unlimited"
    };
    let address = Address::new("turn-limit", "recovery", lane).unwrap();
    let profile = ExecutionProfile::new(None, None, limit, 0).unwrap();
    let prompt = Prompt::new(address.clone(), "first", "system", "work")
        .unwrap()
        .with_profile(profile);
    let accepted = store.accept(&prompt).await.unwrap();
    let claim = store
        .claim_operation(accepted.operation.id, "crashed", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store
        .record(
            &claim.lease,
            &Event::Compacted {
                summary: "work so far".into(),
                steps: RESPONSES,
            },
        )
        .await
        .unwrap();
    store
        .heartbeat(&claim.lease, Duration::from_millis(1))
        .await
        .unwrap();
    store.close().await;
    tokio::time::sleep(Duration::from_millis(5)).await;

    let store = DurableStore::connect(url).await.unwrap();
    let replacement = if limit.is_some() { None } else { Some(1) };
    let runtime = Runtime::new(store.clone());
    let result = runtime
        .run_operation(
            accepted.operation.id,
            &Resuming(limit),
            &Registry::default(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_outcome(result.outcome.as_ref().unwrap(), limit);
    check_next(&store, address, replacement).await;
    store.close().await;
}

fn assert_outcome(outcome: &Outcome, limit: Option<usize>) {
    match limit {
        None => assert_eq!(
            outcome,
            &Outcome::Completed {
                text: "done".into()
            }
        ),
        Some(_) => assert!(
            matches!(outcome, Outcome::Failed { code, .. } if code == "step_budget_exhausted")
        ),
    }
}

async fn check_next(store: &DurableStore, address: Address, limit: Option<usize>) {
    let prompt = Prompt::new(address, "next", "system", "new work")
        .unwrap()
        .with_profile(ExecutionProfile::new(None, None, limit, 0).unwrap());
    let Admitted::Operation(accepted) = store.admit_with_tools(&prompt, &[]).await.unwrap() else {
        panic!("settled lane accepts the next operation");
    };
    let claim = store
        .claim_operation(accepted.operation.id, "next", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claim.thread.profile().max_turns(), limit);
    store
        .settle(
            &claim.lease,
            &Outcome::Completed {
                text: "next done".into(),
            },
        )
        .await
        .unwrap();
}

struct Resuming(Option<usize>);

impl Model for Resuming {
    async fn generate(&self, thread: &Thread, _: &[serde_json::Value]) -> ModelResponse {
        assert_eq!(thread.profile().max_turns(), self.0);
        assert_eq!(thread.steps(), RESPONSES);
        assert!(matches!(
            thread.messages().first(),
            Some(Message::Summary(_))
        ));
        ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: "done".into(),
            tool_calls: Vec::new(),
            stop: StopReason::EndTurn,
        }
    }
}
