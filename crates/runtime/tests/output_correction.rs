#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
// Scenario tables keep assertions alongside each crash boundary and candidate.
#![allow(clippy::too_many_lines)]

//! Durable output correction is bounded independently of process lifetime.
use std::sync::atomic::{AtomicUsize, Ordering};

use agent::{Message, Model, ModelResponse, Registry, StopReason, Thread};
use brainstone_runtime::{Address, MemoryStore, Prompt, Runtime, Store};

struct Correcting(AtomicUsize);

impl Model for Correcting {
    async fn generate(&self, thread: &Thread, _: &[serde_json::Value]) -> ModelResponse {
        self.0.fetch_add(1, Ordering::SeqCst);
        answer(if matches!(thread.last(), Message::OutputCorrection(_)) {
            "corrected"
        } else {
            "invalid"
        })
    }

    async fn output_feedback(&self, _: &Thread) -> Option<String> {
        Some("repair syntax".to_owned())
    }
}

#[tokio::test]
async fn correction_is_journaled_once_before_terminal_delivery() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let address = Address::new("test", "channel", "thread").unwrap();
    let prompt = Prompt::new(address, "event", "system", "draw").unwrap();
    runtime.accept(&prompt).await.unwrap();
    let model = Correcting(AtomicUsize::new(0));
    let result = runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result.outcome,
        Some(agent::Outcome::Completed {
            text: "corrected".to_owned()
        })
    );
    assert_eq!(model.0.load(Ordering::SeqCst), 2);
    let follow = Prompt::new(prompt.address().clone(), "follow", "system", "continue").unwrap();
    runtime.accept(&follow).await.unwrap();
    let claimed = store
        .claim("reader", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        claimed
            .thread
            .messages()
            .iter()
            .filter(|message| matches!(message, Message::OutputCorrection(_)))
            .count(),
        1
    );
}

fn answer(text: &str) -> ModelResponse {
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: text.to_owned(),
        tool_calls: Vec::new(),
        stop: StopReason::EndTurn,
    }
}

// Table-driven crash boundaries share the same replay path as normal execution.
#[tokio::test]
async fn restart_resumes_each_correction_boundary_without_reopening_a_finished_attempt() {
    for boundary in 0..=2 {
        let store = MemoryStore::default();
        let address = Address::new("test", "channel", "thread").unwrap();
        let prompt = Prompt::new(address, "event", "system", "draw").unwrap();
        store.accept(&prompt).await.unwrap();
        let ttl = std::time::Duration::from_millis(3);
        let claim = store.claim("crashed", ttl).await.unwrap().unwrap();
        let messages = [
            Message::Assistant(answer("invalid")),
            Message::OutputCorrection("repair syntax".to_owned()),
            Message::Assistant(answer("corrected")),
        ];
        for message in messages.iter().take(boundary + 1) {
            store
                .record(
                    &claim.lease,
                    &agent::Event::Appended {
                        message: message.clone(),
                    },
                )
                .await
                .unwrap();
        }
        drop(claim);
        tokio::time::sleep(ttl * 2).await;
        let runtime = Runtime::new(store.clone());
        let model = Correcting(AtomicUsize::new(0));
        let result = runtime
            .run_next(&model, &Registry::default())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.outcome,
            Some(agent::Outcome::Completed {
                text: "corrected".to_owned()
            })
        );
        assert_eq!(model.0.load(Ordering::SeqCst), usize::from(boundary < 2));
        assert!(
            runtime
                .run_next(&model, &Registry::default())
                .await
                .unwrap()
                .is_none()
        );
    }
}

#[tokio::test]
async fn correction_does_not_reset_or_exceed_the_response_budget() {
    let runtime = Runtime::new(MemoryStore::default());
    let address = Address::new("test", "channel", "thread").unwrap();
    let profile = agent::ExecutionProfile::new(None, None, 1, 0).unwrap();
    let prompt = Prompt::new(address, "event", "system", "draw")
        .unwrap()
        .with_profile(profile);
    runtime.accept(&prompt).await.unwrap();
    let model = Correcting(AtomicUsize::new(0));
    let result = runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(model.0.load(Ordering::SeqCst), 1);
    assert_eq!(
        result.outcome,
        Some(agent::Outcome::Completed {
            text: "invalid".to_owned()
        })
    );
}
