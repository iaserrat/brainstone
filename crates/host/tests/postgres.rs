#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Opt-in production-store proof. The default test suite never opens Postgres.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use agent::{Model, ModelResponse, Outcome, Registry, StopReason, Thread};
use brainstone_host::{Admission, Config, Host};
use runtime::{Address, OperationState, PostgresStore, Prompt, Runtime};
use tokio::sync::Notify;

struct Answer;

impl Model for Answer {
    async fn generate(&self, _thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: "durable host answer".to_owned(),
            tool_calls: Vec::new(),
            stop: StopReason::EndTurn,
        }
    }
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn named_execution_and_retry_use_one_postgres_outcome() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("connect");
    let runtime = Runtime::new(store.clone());
    let host = Host::new(runtime, Answer, Registry::default());
    let address = Address::new("test", &unique_suffix(), "main").expect("address");
    let prompt = Prompt::new(address, "ci-run", "system", "hello").expect("prompt");

    let id = Admission::accept(&host, &prompt).await.expect("admitted");
    let completed = host.execute(id).await.expect("executed");
    let retry = Admission::accept(&host, &prompt).await.expect("retried");
    let restored = host.execute(retry).await.expect("restored");

    assert_eq!(retry, id);
    assert_eq!(restored, completed);
    assert!(matches!(restored.outcome, Some(Outcome::Completed { .. })));
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn host_accepts_runs_and_shuts_down_on_postgres() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("connect");
    let runtime = Runtime::new(store.clone());
    let config = Config::new(Duration::from_millis(10)).expect("config");
    let host = Host::with_config(runtime.clone(), Answer, Registry::default(), config);
    let worker = host.clone();
    let running = tokio::spawn(async move { worker.run().await });
    let address = Address::new("host-contract", &unique_suffix(), "lane").expect("address");
    let prompt = Prompt::new(address, "request", "system", "hello").expect("prompt");

    let accepted = host.accept(&prompt).await.expect("durable acceptance");
    let operation = wait_for_terminal(&runtime, accepted.operation.id).await;

    assert_eq!(operation.state, OperationState::Completed);
    host.shutdown().await;
    running.await.expect("worker joined").expect("worker exits");
    store.close().await;
}

/// Reproduces the 2026-09 restart stalls: Postgres stalled longer than the
/// pool's 5 s acquire timeout, idle claims queued behind 10 connections timed
/// out, and the first `store_unavailable` stopped every lane on the replica.
#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn an_idle_host_outlives_a_store_stall_past_the_acquire_timeout() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("connect");
    let runtime = Runtime::new(store.clone());
    let host = Host::new(runtime.clone(), Answer, Registry::default());
    let worker = host.clone();
    let running = tokio::spawn(async move { worker.run().await });

    stall_claims(&url, Duration::from_secs(7)).await;

    let address = Address::new("host-stall", &unique_suffix(), "lane").expect("address");
    let prompt = Prompt::new(address, "request", "system", "hello").expect("prompt");
    let accepted = host.accept(&prompt).await.expect("durable acceptance");
    let operation = wait_for_terminal(&runtime, accepted.operation.id).await;
    assert_eq!(operation.state, OperationState::Completed);
    host.shutdown().await;
    running.await.expect("worker joined").expect("worker exits");
    store.close().await;
}

/// The same stall while an operation runs: its next write cannot get a pooled
/// connection, the worker lets the operation go, and once its lease expires a
/// worker of the same process resumes it from its records.
#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn a_store_stall_mid_operation_resumes_it_in_the_same_process() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("connect");
    let lease = runtime::Config::new("stall-worker", Duration::from_secs(2)).expect("config");
    let runtime = Runtime::with_config(store.clone(), lease);
    let model = BlockOnce::new();
    let host = Host::new(runtime.clone(), model.clone(), Registry::default());
    let worker = host.clone();
    let running = tokio::spawn(async move { worker.run().await });
    let address = Address::new("host-stall", &unique_suffix(), "lane").expect("address");
    let prompt = Prompt::new(address, "request", &model.system, "hello").expect("prompt");
    let accepted = host.accept(&prompt).await.expect("durable acceptance");
    model.entered.notified().await;

    let stall = tokio::spawn(async move { stall_claims(&url, Duration::from_secs(8)).await });
    // Idle claims pin every pooled connection before the model's answer needs one.
    tokio::time::sleep(Duration::from_secs(1)).await;
    model.release.notify_one();
    stall.await.expect("stall joined");

    let operation = wait_for_terminal(&runtime, accepted.operation.id).await;
    assert_eq!(operation.state, OperationState::Completed);
    let calls = model.calls.load(Ordering::SeqCst);
    assert_eq!(calls, 2, "resumed after the stall");
    assert!(!running.is_finished(), "the stall stopped the host");
    host.shutdown().await;
    running.await.expect("worker joined").expect("worker exits");
    store.close().await;
}

/// Blocks its first call until released, then answers every call at once.
#[derive(Clone, Default)]
struct BlockOnce {
    system: String,
    entered: Arc<Notify>,
    release: Arc<Notify>,
    calls: Arc<AtomicUsize>,
}

impl BlockOnce {
    fn new() -> Self {
        Self {
            system: unique_suffix(),
            ..Self::default()
        }
    }
}

impl Model for BlockOnce {
    async fn generate(&self, thread: &Thread, tools: &[serde_json::Value]) -> ModelResponse {
        // Other integration tests may leave recoverable operations in this database.
        if thread.system != self.system {
            return Answer.generate(thread, tools).await;
        }
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.entered.notify_one();
            self.release.notified().await;
        }
        Answer.generate(thread, tools).await
    }
}

/// Every claim first reads `scheduled_work`, so an exclusive lock on it stalls
/// all of them at once, the way a frozen database does.
async fn stall_claims(url: &str, stall: Duration) {
    use sqlx::Connection as _;
    let mut connection = sqlx::PgConnection::connect(url).await.expect("connect");
    let mut transaction = connection.begin().await.expect("begin");
    sqlx::query("LOCK TABLE scheduled_work IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *transaction)
        .await
        .expect("lock");
    tokio::time::sleep(stall).await;
    transaction.commit().await.expect("release");
}

async fn wait_for_terminal(
    runtime: &Runtime<PostgresStore>,
    id: runtime::OperationId,
) -> runtime::Operation {
    for _ in 0..100 {
        let operation = runtime
            .operation(id)
            .await
            .expect("operation read")
            .expect("operation exists");
        if matches!(
            operation.state,
            OperationState::Completed | OperationState::Failed
        ) {
            return operation;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("operation did not finish")
}

fn unique_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    format!("host-{}-{nanos}", std::process::id())
}
