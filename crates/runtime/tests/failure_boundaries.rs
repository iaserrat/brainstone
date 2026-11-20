#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Fault campaigns exercise real backend commits around external effects and recovery.
#![allow(clippy::too_many_lines)] // Table-driven crash/outage scenarios.
use agent::{
    Event, Message, Model, ModelResponse, Outcome, Registry, Sink, StopReason, Thread, Tool,
    ToolCall, ToolError, ToolOutput, ToolRequest,
};
use brainstone_runtime::{Address, Config, DurableStore, Prompt, Runtime, Store};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
#[path = "support/database.rs"]
mod database;
#[path = "support/fault_store.rs"]
mod faults;
use faults::{Boundary, FaultStore};
use tokio::sync::Notify;

// Normal commits must not race the CI scheduler; faults expire captured leases explicitly.
const LEASE: Duration = Duration::from_secs(30);
const HEARTBEAT_LEASE: Duration = Duration::from_secs(3);
// Reproduce a claim acknowledgement slower than the former one-second test lease.
const SLOW_CLAIM_ACK: Duration = Duration::from_millis(1100);

#[tokio::test]
async fn sqlite_recovers_external_effects_streams_and_database_outages() {
    let database = database::Database::sqlite();
    check(&database.url).await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_recovers_external_effects_streams_and_database_outages() {
    let database = database::Database::postgres().await;
    check(&database.url).await;
    database.close().await;
}

async fn check(url: &str) {
    for (index, boundary) in [
        Boundary::ToolResults,
        Boundary::ModelAcknowledgement,
        Boundary::Terminal,
    ]
    .into_iter()
    .enumerate()
    {
        external_effects(url, index, boundary).await;
    }
    interrupted_stream(url).await;
    uncertain_effect_steered(url).await;
    steering_commit(url).await;
    delivery_crash(url).await;
}

async fn uncertain_effect_steered(url: &str) {
    let store = DurableStore::connect(url).await.unwrap();
    let broken = FaultStore::new(store.clone(), Boundary::ToolResults);
    let service = Arc::new(ExternalService::default());
    let tools = Registry::new(vec![service.clone()]);
    let original = prompt("uncertain-steer", "one", "perform effects");
    let id = runtime(broken.clone())
        .accept(&original)
        .await
        .unwrap()
        .operation
        .id;
    let model = Batch {
        calls: AtomicUsize::new(0),
    };
    assert!(
        runtime(broken.clone())
            .run_next(&model, &tools)
            .await
            .is_err()
    );
    store
        .admit_with_tools(&prompt("uncertain-steer", "two", "cancel effects"), &[])
        .await
        .unwrap();
    broken.expire_lease().await;
    store.close().await;
    let store = DurableStore::connect(url).await.unwrap();
    let operation = runtime(store.clone())
        .run_next(&Cancelled, &tools)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(operation.id, id);
    assert_eq!(
        operation.outcome,
        Some(Outcome::Completed {
            text: "cancelled".into()
        })
    );
    assert_eq!(
        service.calls.lock().unwrap().values().sum::<usize>(),
        2,
        "uncertain effects were retried after cancellation"
    );
    store.close().await;
}

struct Cancelled;
impl Model for Cancelled {
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        assert_eq!(thread.last(), &Message::User("cancel effects".into()));
        let Message::ToolResults(results) = &thread.messages()[2] else {
            panic!("closed original batch")
        };
        assert!(results.iter().all(
            |result| serde_json::from_str::<Value>(&result.content).unwrap()["error"]
                == brainstone_runtime::CODE_TOOL_SUPERSEDED
        ));
        answer("cancelled")
    }
}

fn runtime<S: Store>(store: S) -> Runtime<S> {
    Runtime::with_config(store, Config::new("fault-campaign", LEASE).unwrap())
}

fn prompt(lane: &str, id: &str, text: &str) -> Prompt {
    Prompt::new(
        Address::new("faults", "session", lane).unwrap(),
        id,
        "system",
        text,
    )
    .unwrap()
}

async fn external_effects(url: &str, index: usize, boundary: Boundary) {
    let store = DurableStore::connect(url).await.unwrap();
    let broken = FaultStore::new(store.clone(), boundary);
    let model = Batch {
        calls: AtomicUsize::new(0),
    };
    // This service outlives both runtime instances and atomically deduplicates keys.
    let service = Arc::new(ExternalService::default());
    let tools = Registry::new(vec![service.clone()]);
    let prompt = prompt(&format!("effect-{index}"), "one", "run the batch");
    let operation = runtime(broken.clone())
        .accept(&prompt)
        .await
        .unwrap()
        .operation
        .id;
    assert_eq!(
        runtime(broken.clone())
            .run_next(&model, &tools)
            .await
            .unwrap_err()
            .code(),
        "store_unavailable"
    );
    assert!(
        store
            .operation(operation)
            .await
            .unwrap()
            .unwrap()
            .outcome
            .is_none()
    );
    broken.expire_lease().await;
    store.close().await;
    let reopened = DurableStore::connect(url).await.unwrap();
    let result = runtime(reopened.clone())
        .run_next(&model, &tools)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.id, operation);
    assert_eq!(
        result.outcome,
        Some(Outcome::Completed {
            text: "done".into()
        })
    );
    {
        let calls = service.calls.lock().unwrap();
        assert_eq!(calls.len(), 2, "one external effect per unique tool call");
        let expected_attempts = if matches!(boundary, Boundary::ToolResults) {
            4
        } else {
            2
        };
        assert_eq!(calls.values().sum::<usize>(), expected_attempts);
    }
    assert_eq!(
        model.calls.load(Ordering::SeqCst),
        2,
        "committed model result was repeated"
    );
    reopened.close().await;
}

#[derive(Default)]
struct ExternalService {
    calls: Mutex<HashMap<String, usize>>,
    pause: AtomicBool,
    entered: Notify,
    release: Notify,
}
impl Tool for ExternalService {
    fn name(&self) -> &'static str {
        "External"
    }
    fn definition(&self) -> Value {
        json!({"name":"External","description":"Idempotent external effect","input_schema":{"type":"object"}})
    }
    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            let key = request.idempotency_key.expect("durable retry key");
            {
                let mut calls = self.calls.lock().unwrap();
                *calls.entry(key).or_default() += 1;
            }
            if self.pause.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Ok(ToolOutput::from("effect committed once".to_owned()))
        })
    }
}

#[tokio::test]
async fn sqlite_writer_outage_after_an_external_effect_recovers_without_a_second_effect() {
    let database = database::Database::sqlite();
    let store = DurableStore::connect(&database.url).await.unwrap();
    let service = Arc::new(ExternalService::default());
    service.pause.store(true, Ordering::SeqCst);
    let model = Arc::new(Batch {
        calls: AtomicUsize::new(0),
    });
    let tools = Registry::new(vec![service.clone()]);
    let runtime = Runtime::with_config(
        store.clone(),
        Config::new("outage", Duration::from_secs(2)).unwrap(),
    );
    let id = runtime
        .accept(&prompt("real-outage", "one", "batch"))
        .await
        .unwrap()
        .operation
        .id;
    let running = tokio::spawn({
        let (model, tools) = (model.clone(), tools.clone());
        async move { runtime.run_next(model.as_ref(), &tools).await }
    });
    service.entered.notified().await;
    let locks = sqlx::SqlitePool::connect(&database.url).await.unwrap();
    let transaction = locks.begin_with("BEGIN IMMEDIATE").await.unwrap();
    service.release.notify_one();
    let error = tokio::time::timeout(Duration::from_secs(8), running)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(matches!(error.code(), "store_unavailable" | "lease_lost"));
    transaction.rollback().await.unwrap();
    assert!(
        store
            .operation(id)
            .await
            .unwrap()
            .unwrap()
            .outcome
            .is_none()
    );
    let operation = self::runtime(store.clone())
        .run_next(model.as_ref(), &tools)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(operation.id, id);
    {
        let calls = service.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls.values().sum::<usize>(),
            3,
            "first effect retried with its original key"
        );
    }
    locks.close().await;
    store.close().await;
}

struct Batch {
    calls: AtomicUsize,
}
impl Model for Batch {
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if matches!(thread.last(), Message::User(_)) {
            return ModelResponse {
                tool_calls: ["a", "b"]
                    .into_iter()
                    .map(|id| ToolCall {
                        id: id.into(),
                        name: "External".into(),
                        args: json!({}),
                    })
                    .collect(),
                stop: StopReason::ToolUse,
                ..answer("")
            };
        }
        answer("done")
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

struct BrokenStream;
impl Model for BrokenStream {
    async fn generate(&self, _: &Thread, _: &[Value]) -> ModelResponse {
        unreachable!("streaming")
    }
    async fn generate_stream<S: Sink + Send>(
        &self,
        _: &Thread,
        _: &[Value],
        sink: &mut S,
    ) -> Result<ModelResponse, S::Error> {
        sink.emit(Event::ModelDelta {
            delta: "uncommitted partial output".into(),
        })
        .await?;
        std::future::pending().await
    }
}

struct Recovery;
impl Model for Recovery {
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        assert!(
            thread
                .messages()
                .iter()
                .all(|message| matches!(message, Message::User(_))),
            "partial output entered replay"
        );
        answer("recovered")
    }
}

async fn interrupted_stream(url: &str) {
    let store = DurableStore::connect(url).await.unwrap();
    let broken = FaultStore::new(store.clone(), Boundary::Heartbeat);
    let prompt = prompt("stream", "one", "start");
    let id = runtime(broken.clone())
        .accept(&prompt)
        .await
        .unwrap()
        .operation
        .id;
    assert_eq!(
        Runtime::with_config(
            broken.clone(),
            Config::new("heartbeat-fault", HEARTBEAT_LEASE).unwrap()
        )
        .run_next(&BrokenStream, &Registry::default())
        .await
        .unwrap_err()
        .code(),
        "store_unavailable"
    );
    assert!(
        store
            .operation(id)
            .await
            .unwrap()
            .unwrap()
            .outcome
            .is_none()
    );
    broken.expire_lease().await;
    store.close().await;
    let store = DurableStore::connect(url).await.unwrap();
    let operation = runtime(store.clone())
        .run_next(&Recovery, &Registry::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(operation.id, id);
    assert_eq!(
        operation.outcome,
        Some(Outcome::Completed {
            text: "recovered".into()
        })
    );
    store.close().await;
}

async fn steering_commit(url: &str) {
    let store = DurableStore::connect(url).await.unwrap();
    let broken = FaultStore::new(store.clone(), Boundary::SteeringAcknowledgement);
    let runtime = runtime(broken.clone());
    let id = runtime
        .accept(&prompt("steer", "one", "original"))
        .await
        .unwrap()
        .operation
        .id;
    let claimed = store
        .claim_operation(id, "before-crash", LEASE)
        .await
        .unwrap()
        .unwrap();
    store
        .admit_with_tools(&prompt("steer", "two", "correction ☕"), &[])
        .await
        .unwrap();
    store
        .heartbeat(&claimed.lease, Duration::ZERO)
        .await
        .unwrap();
    assert_eq!(
        runtime
            .run_next(&Recovery, &Registry::default())
            .await
            .unwrap_err()
            .code(),
        "store_unavailable"
    );
    broken.expire_lease().await;
    store.close().await;
    let store = DurableStore::connect(url).await.unwrap();
    let claim = store
        .claim_operation(id, "recovered", LEASE)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        claim.thread.messages(),
        &[
            Message::User("original".into()),
            Message::User("correction ☕".into())
        ]
    );
    assert_eq!(store.has_steering(&claim.lease).await.unwrap(), Some(false));
    store
        .settle(
            &claim.lease,
            &Outcome::Completed {
                text: "recovered".into(),
            },
        )
        .await
        .unwrap();
    store.close().await;
}

async fn delivery_crash(url: &str) {
    use brainstone_runtime::{DeliveryCheckpoint, DeliveryClaim, DeliveryStore, DeliveryTarget};
    let store = DurableStore::connect(url).await.unwrap();
    let prompt = prompt("delivery", "one", "deliver")
        .with_delivery(DeliveryTarget::new("fake-surface", json!({"thread":"one"})).unwrap());
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let claim = store
        .claim_operation(id, "executor", LEASE)
        .await
        .unwrap()
        .unwrap();
    store
        .settle(
            &claim.lease,
            &Outcome::Completed {
                text: "durable reply".into(),
            },
        )
        .await
        .unwrap();
    let request = DeliveryClaim::new("fake-surface", "old-deliverer", LEASE, 100).unwrap();
    let old = store.claim_delivery(&request).await.unwrap().unwrap();
    store
        .save_delivery_stream(&old.lease, Some("external-receipt"))
        .await
        .unwrap();
    store
        .renew_delivery(&old.lease, Duration::ZERO)
        .await
        .unwrap();
    store.close().await;
    let store = DurableStore::connect(url).await.unwrap();
    let request = DeliveryClaim::new("fake-surface", "recovered-deliverer", LEASE, 100).unwrap();
    let recovered = store.claim_delivery(&request).await.unwrap().unwrap();
    assert_eq!(recovered.external_id.as_deref(), Some("external-receipt"));
    assert_eq!(
        store
            .checkpoint_delivery(&DeliveryCheckpoint::new(
                old.lease,
                old.events.last().unwrap().sequence,
                None
            ))
            .await
            .unwrap_err()
            .code(),
        "lease_lost"
    );
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(
            recovered.lease,
            recovered.events.last().unwrap().sequence,
            Some("external-receipt".into()),
        ))
        .await
        .unwrap();
    assert!(store.claim_delivery(&request).await.unwrap().is_none());
    store.close().await;
}

#[tokio::test]
async fn fault_campaign_tolerates_a_slow_claim_acknowledgement() {
    let database = database::Database::sqlite();
    let store = DurableStore::connect(&database.url).await.unwrap();
    let delayed =
        FaultStore::new(store.clone(), Boundary::ToolResults).with_claim_delay(SLOW_CLAIM_ACK);
    let runtime = runtime(delayed);
    runtime
        .accept(&prompt("slow-claim", "one", "start"))
        .await
        .unwrap();
    let operation = runtime
        .run_next(&Recovery, &Registry::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        operation.outcome,
        Some(Outcome::Completed {
            text: "recovered".into()
        })
    );
    store.close().await;
}
