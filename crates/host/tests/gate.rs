#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Admission-gate behavior through the public Admission seam.

use std::sync::{Arc, Mutex};

use brainstone_host::{Admission, GatedAdmission};
use runtime::{Address, MemoryStore, OperationId, Prompt, Store};

#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<Prompt>>>);

impl Admission for Recorder {
    async fn accept(&self, prompt: &Prompt) -> Result<OperationId, brainstone_host::Error> {
        self.0
            .lock()
            .expect("recorder poisoned")
            .push(prompt.clone());
        Ok(OperationId::new(1).expect("operation id"))
    }
}

fn prompt(actor: Option<&str>) -> Prompt {
    prompt_for("event-1", actor)
}

fn prompt_for(request_id: &str, actor: Option<&str>) -> Prompt {
    let address = Address::new("test", "C1", "thread-1").expect("address");
    let prompt = Prompt::new(address, request_id, "be useful", "héllo ☕").expect("prompt");
    match actor {
        Some(actor) => prompt.with_actor(actor),
        None => prompt,
    }
}

fn allowed(actors: &[&str]) -> Vec<String> {
    actors.iter().map(|actor| (*actor).to_owned()).collect()
}

#[tokio::test]
async fn an_unconfigured_gate_admits_everything_untouched() {
    let recorder = Recorder::default();
    let gate = GatedAdmission::new(recorder.clone(), MemoryStore::default(), None, None);

    gate.accept(&prompt(None)).await.expect("admitted");

    let accepted = recorder.0.lock().expect("recorder poisoned");
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0], prompt(None));
}

#[tokio::test]
async fn the_allowlist_rejects_unlisted_and_anonymous_actors() {
    let recorder = Recorder::default();
    let gate = GatedAdmission::new(
        recorder.clone(),
        MemoryStore::default(),
        Some(allowed(&["U1"])),
        None,
    );

    gate.preflight(Some("U1")).await.expect("listed preflight");
    gate.accept(&prompt(Some("U1"))).await.expect("listed");
    assert_eq!(
        gate.preflight(Some("U9")).await.unwrap_err().code(),
        "admission_denied"
    );
    let unlisted = gate
        .accept(&prompt(Some("U9")))
        .await
        .expect_err("unlisted");
    assert_eq!(unlisted.code(), "admission_denied");
    let anonymous = gate.accept(&prompt(None)).await.expect_err("anonymous");
    assert_eq!(anonymous.code(), "admission_denied");
    assert_eq!(recorder.0.lock().expect("recorder poisoned").len(), 1);
}

#[tokio::test]
async fn an_existing_request_bypasses_changed_admission_policy() {
    let store = MemoryStore::default();
    let existing = prompt(Some("U1"));
    store.accept(&existing).await.expect("already durable");
    let recorder = Recorder::default();
    let gate = GatedAdmission::new(recorder, store, Some(allowed(&["U2"])), Some(0));

    let operation = gate.accept(&existing).await.expect("idempotent retry");

    assert_eq!(operation.get(), 1);
}

#[tokio::test]
async fn the_daily_limit_rejects_an_actor_at_its_budget() {
    let store = MemoryStore::default();
    store
        .accept(&prompt(Some("U1")))
        .await
        .expect("prior operation accepted");
    let recorder = Recorder::default();
    let gate = GatedAdmission::new(recorder.clone(), store, None, Some(1));

    assert_eq!(
        gate.preflight(Some("U1")).await.unwrap_err().code(),
        "admission_quota_exceeded"
    );
    let error = gate
        .accept(&prompt_for("event-2", Some("U1")))
        .await
        .expect_err("at the limit");
    assert_eq!(error.code(), "admission_quota_exceeded");

    gate.accept(&prompt_for("event-2", Some("U2")))
        .await
        .expect("another actor is under its own budget");
}
