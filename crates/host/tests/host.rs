#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Public host lifecycle contract tests; setup is intentionally explicit.
#![allow(clippy::too_many_lines)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{io, io::Write};

use agent::{Event, Model, ModelResponse, Outcome, Registry, StopReason, Thread};
use brainstone_host::{Admission, Config as HostConfig, Host};
use runtime::{
    Accepted, Address, Claimed, Compaction, Error, Lease, MemoryStore, Operation, OperationId,
    Prompt, Runtime, Store,
};
use tokio::sync::Notify;
use tracing_subscriber::fmt::MakeWriter;

#[test]
fn unsafe_poll_intervals_are_rejected_with_a_stable_code() {
    let intervals = [
        Duration::ZERO,
        Duration::from_millis(1),
        Duration::from_mins(2),
    ];

    for interval in intervals {
        let error = HostConfig::new(interval).expect_err("unsafe interval");
        assert_eq!(error.code(), "invalid_input");
        assert_eq!(error.to_string(), "invalid poll_interval");
    }
}

#[tokio::test]
async fn ingress_acknowledges_only_after_the_prompt_is_durable() {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let store = AcceptStore {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    };
    let host = Host::new(Runtime::new(store), NoopModel, Registry::default());
    let accepting = host.clone();
    let task = tokio::spawn(async move { accepting.accept(&prompt()).await });
    entered.notified().await;

    assert!(!task.is_finished());
    release.notify_one();
    let accepted = task.await.expect("task joined").expect("accepted");
    assert_eq!(accepted.operation.id.get(), 1);
}

#[tokio::test]
async fn surface_admission_returns_the_durable_operation_receipt() {
    let host = Host::new(
        Runtime::new(RunStore::default()),
        NoopModel,
        Registry::default(),
    );

    let receipt = Admission::accept(&host, &prompt()).await.expect("admitted");

    assert_eq!(receipt.get(), 1);
}

#[tokio::test]
async fn shutdown_waits_for_an_in_flight_durable_acceptance() {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let store = AcceptStore {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    };
    let host = Host::new(Runtime::new(store), NoopModel, Registry::default());
    let accepting = host.clone();
    let acceptance = tokio::spawn(async move { accepting.accept(&prompt()).await });
    entered.notified().await;
    let stopping = host.clone();
    let shutdown = tokio::spawn(async move { stopping.shutdown().await });
    tokio::task::yield_now().await;

    assert!(!shutdown.is_finished());
    release.notify_one();
    acceptance.await.expect("accept joined").expect("accepted");
    shutdown.await.expect("shutdown joined");
    assert_eq!(
        host.accept(&prompt()).await.expect_err("closed").code(),
        "host_shutting_down"
    );
}

#[tokio::test]
async fn accepted_work_wakes_an_idle_worker_immediately() {
    let store = RunStore::default();
    let model = Answer::default();
    let called = Arc::clone(&model.called);
    let config = HostConfig::new(Duration::from_mins(1)).expect("host config");
    let host = Host::with_config(
        Runtime::new(store.clone()),
        model,
        Registry::default(),
        config,
    );
    let worker = host.clone();
    let running = tokio::spawn(async move { worker.run().await });
    store.idle.notified().await;

    host.accept(&prompt()).await.expect("accepted");

    tokio::time::timeout(Duration::from_millis(100), called.notified())
        .await
        .expect("acceptance wakes worker");
    store.completed.notified().await;
    running.abort();
}

#[tokio::test]
async fn one_shot_execution_returns_the_named_durable_outcome() {
    let store = RunStore::default();
    let host = Host::new(Runtime::new(store), Answer::default(), Registry::default());
    let accepted = host.accept(&prompt()).await.expect("accepted");

    let operation = host
        .execute(accepted.operation.id)
        .await
        .expect("operation completed");

    assert_eq!(
        operation.outcome,
        Some(Outcome::Completed {
            text: "done".to_owned()
        })
    );
}

#[tokio::test]
async fn one_shot_retry_reuses_the_terminal_outcome_without_model_spend() {
    let model = Answer::default();
    let calls = Arc::clone(&model.calls);
    let host = Host::new(
        Runtime::new(MemoryStore::default()),
        model,
        Registry::default(),
    );
    let first = Admission::accept(&host, &prompt()).await.expect("admitted");
    let completed = host.execute(first).await.expect("completed");

    let retry = Admission::accept(&host, &prompt()).await.expect("retried");
    let restored = host.execute(retry).await.expect("restored");

    assert_eq!(retry, first);
    assert_eq!(restored, completed);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn shutdown_closes_admission_and_wakes_an_idle_worker() {
    let store = RunStore::default();
    let host = Host::new(Runtime::new(store.clone()), NoopModel, Registry::default());
    let worker = host.clone();
    let running = tokio::spawn(async move { worker.run().await });
    store.idle.notified().await;

    host.shutdown().await;

    let result = tokio::time::timeout(Duration::from_millis(100), running)
        .await
        .expect("worker exits")
        .expect("worker joined");
    assert!(result.is_ok());
    let error = host.accept(&prompt()).await.expect_err("admission closed");
    assert_eq!(error.code(), "host_shutting_down");
    assert!(store.state.lock().expect("state poisoned").is_none());
}

/// Regression for replicas that restarted whenever Postgres stalled past the
/// pool's acquire timeout: one failed idle claim stopped every lane. One worker
/// proves the failed loop itself retries; with more, another could serve it.
#[tokio::test(start_paused = true)]
async fn a_brief_store_failure_pauses_a_worker_without_stopping_the_service() {
    let store = RunStore::failing(3, unavailable());
    let config = HostConfig::default().with_workers(1).expect("one worker");
    let host = Host::with_config(
        Runtime::new(store.clone()),
        NoopModel,
        Registry::default(),
        config,
    );
    let worker = tokio::spawn({
        let host = host.clone();
        async move { host.run().await }
    });
    let wait = Duration::from_mins(1);
    tokio::time::timeout(wait, store.idle.notified())
        .await
        .expect("the worker claims again after the failure");

    host.accept(&prompt())
        .await
        .expect("the service still admits");
    tokio::time::timeout(wait, store.completed.notified())
        .await
        .expect("the worker runs the admitted prompt");

    assert!(!worker.is_finished(), "the failure did not stop the host");
    host.shutdown().await;
    worker
        .await
        .expect("worker joined")
        .expect("a brief store failure is not a service failure");
}

/// A stall longer than the lease fences the worker out with `lease_lost`; it
/// leaves that operation to lease recovery and keeps serving other work.
#[tokio::test(start_paused = true)]
async fn a_lost_lease_abandons_the_operation_without_stopping_the_service() {
    let store = RunStore {
        lost_heartbeats: Arc::new(AtomicUsize::new(1)),
        ..RunStore::default()
    };
    let lease = runtime::Config::new("worker", Duration::from_secs(3)).expect("runtime config");
    let host = Host::with_config(
        Runtime::with_config(store.clone(), lease),
        SlowThenAnswer::default(),
        Registry::default(),
        HostConfig::default().with_workers(1).expect("one worker"),
    );
    host.accept(&prompt()).await.expect("accepted");
    let worker = tokio::spawn({
        let host = host.clone();
        async move { host.run().await }
    });
    let wait = Duration::from_mins(1);
    tokio::time::timeout(wait, store.idle.notified())
        .await
        .expect("the worker returns to polling");

    host.accept(&prompt())
        .await
        .expect("the service still admits");
    tokio::time::timeout(wait, store.completed.notified())
        .await
        .expect("the worker runs the next prompt");

    assert!(
        !worker.is_finished(),
        "the lost lease did not stop the host"
    );
    host.shutdown().await;
    worker
        .await
        .expect("worker joined")
        .expect("a lost lease is not a service failure");
}

/// A store that never answers is a broken dependency: the worker
/// gives up, closes admission, and lets the process supervisor restart it.
#[tokio::test(start_paused = true)]
async fn a_store_that_stays_unavailable_closes_admission_before_returning() {
    let host = Host::new(
        Runtime::new(RunStore::failing(usize::MAX, unavailable())),
        NoopModel,
        Registry::default(),
    );
    let started = tokio::time::Instant::now();

    let failure = host.run().await.expect_err("worker fails");

    assert_eq!(failure.code(), "store_unavailable");
    assert_eq!(started.elapsed(), RETRY_BUDGET, "six pauses, then closed");
    let rejected = host.accept(&prompt()).await.expect_err("failed closed");
    assert_eq!(rejected.code(), "host_shutting_down");
}

/// Accepted prompts wake idle workers; they must not end a retry pause early
/// and spend the budget faster than the documented schedule.
#[tokio::test(start_paused = true)]
async fn accepted_prompts_do_not_shorten_the_retry_budget() {
    let host = one_worker(RunStore::failing(usize::MAX, unavailable()));
    let started = tokio::time::Instant::now();
    let worker = tokio::spawn({
        let host = host.clone();
        async move { host.run().await }
    });
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        host.accept(&prompt())
            .await
            .expect("accepted while claims fail");
    }

    let failure = worker.await.expect("worker joined").expect_err("closed");

    assert_eq!(failure.code(), "store_unavailable");
    assert_eq!(started.elapsed(), RETRY_BUDGET);
}

/// Shutdown ends a retry pause at once instead of waiting it out.
#[tokio::test(start_paused = true)]
async fn shutdown_ends_a_retry_pause_at_once() {
    let host = one_worker(RunStore::failing(usize::MAX, unavailable()));
    let worker = tokio::spawn({
        let host = host.clone();
        async move { host.run().await }
    });
    // Past 3.75 s of earlier pauses, inside the 5 s one.
    tokio::time::sleep(Duration::from_secs(4)).await;

    host.shutdown().await;

    tokio::time::timeout(Duration::from_millis(1), worker)
        .await
        .expect("the pause ends at shutdown")
        .expect("worker joined")
        .expect("a shutdown is not a failure");
}

/// 250 ms + 500 ms + 1 s + 2 s + 5 s + 10 s of pauses before a worker gives up.
const RETRY_BUDGET: Duration = Duration::from_millis(18_750);

fn one_worker(store: RunStore) -> Host<RunStore, NoopModel> {
    let config = HostConfig::default().with_workers(1).expect("one worker");
    Host::with_config(Runtime::new(store), NoopModel, Registry::default(), config)
}

/// Only an unreachable store or a lost lease is waited out. A record no retry
/// can interpret, or a write the database refuses, fails the same way on every
/// attempt and stops the host on the first one.
#[tokio::test]
async fn an_error_no_retry_can_fix_stops_the_host_without_a_pause() {
    let cases = [
        Error::CorruptRecord {
            message: "unknown event schema".to_owned(),
        },
        Error::StoreFailed {
            message: "invalid byte sequence for encoding \"UTF8\": 0x00".to_owned(),
        },
    ];
    for error in cases {
        let code = error.code();
        let host = Host::new(
            Runtime::new(RunStore::failing(1, error)),
            NoopModel,
            Registry::default(),
        );

        let failure = tokio::time::timeout(Duration::from_millis(100), host.run())
            .await
            .expect("no retry pause")
            .expect_err("worker fails");

        assert_eq!(failure.code(), code);
    }
}

fn unavailable() -> Error {
    Error::StoreUnavailable {
        message: "pool timed out while waiting for an open connection".to_owned(),
    }
}

#[tokio::test]
async fn a_durable_model_failure_is_logged_with_its_stable_code() {
    let logs = Logs::default();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .without_time()
        .with_writer(logs.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let store = RunStore::default();
    let host = Host::new(
        Runtime::new(store.clone()),
        FailureModel,
        Registry::default(),
    );
    host.accept(&prompt()).await.unwrap();
    let worker = tokio::spawn({
        let host = host.clone();
        async move { host.run().await }
    });
    store.completed.notified().await;
    tokio::task::yield_now().await;
    host.shutdown().await;
    worker.await.unwrap().unwrap();

    let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(output.contains("operation failed"));
    assert!(output.contains("invalid_request"));
}

/// The same provider failure that `a_durable_model_failure_is_logged_with_its
/// _stable_code` reports politely, arriving during automatic summarization.
/// Before the fix this returned `Err` from `Host::run` and stopped the replica
/// for every lane on it, so the second operation here is the whole point.
#[tokio::test]
async fn a_failed_compaction_ends_the_turn_without_stopping_the_service() {
    let store = RunStore::default();
    let policy = runtime::CompactionPolicy::new(1, "summarize").expect("compaction policy");
    let config = runtime::Config::new("worker", Duration::from_secs(30))
        .expect("runtime config")
        .with_compaction(policy);
    let model = ThrottleThenAnswer::default();
    let host = Host::new(
        Runtime::with_config(store.clone(), config),
        model,
        Registry::default(),
    );
    host.accept(&prompt()).await.expect("accepted");
    let worker = tokio::spawn({
        let host = host.clone();
        async move { host.run().await }
    });

    settled(&store, "a failed summary settles its own operation").await;
    let failed = store.state.lock().expect("state poisoned").clone();
    let Some((runtime::OperationState::Failed, Some(Outcome::Failed { code, .. }))) = failed else {
        panic!("the turn ends terminally: {failed:?}");
    };
    assert_eq!(code, "provider_unavailable");

    host.accept(&prompt())
        .await
        .expect("the replica still accepts");
    settled(&store, "the replica still serves other work").await;

    host.shutdown().await;
    worker
        .await
        .expect("worker joined")
        .expect("a failed summary does not stop every other lane on this replica");
    let state = store.state.lock().expect("state poisoned");
    assert!(
        matches!(
            state.as_ref(),
            Some((runtime::OperationState::Completed, _))
        ),
        "the next operation completed: {state:?}"
    );
}

/// The compaction retry sleeps between attempts, so this waits past that.
async fn settled(store: &RunStore, reason: &str) {
    tokio::time::timeout(Duration::from_secs(5), store.completed.notified())
        .await
        .unwrap_or_else(|_| panic!("{reason}"));
}

#[tokio::test]
async fn periodic_polling_finds_work_accepted_by_another_replica() {
    let store = RunStore::default();
    let model = Answer::default();
    let called = Arc::clone(&model.called);
    let config = HostConfig::new(Duration::from_millis(10)).expect("host config");
    let host = Host::with_config(
        Runtime::new(store.clone()),
        model,
        Registry::default(),
        config,
    );
    let worker = host.clone();
    let running = tokio::spawn(async move { worker.run().await });
    store.idle.notified().await;

    store.enqueue();

    tokio::time::timeout(Duration::from_millis(100), called.notified())
        .await
        .expect("poll finds external work");
    store.completed.notified().await;
    host.shutdown().await;
    running.await.expect("worker joined").expect("worker exits");
}

#[tokio::test]
async fn shutdown_waits_for_an_in_flight_operation_to_finish() {
    let store = RunStore::default();
    let model = BlockingModel::default();
    let entered = Arc::clone(&model.entered);
    let release = Arc::clone(&model.release);
    let host = Host::new(Runtime::new(store.clone()), model, Registry::default());
    host.accept(&prompt()).await.expect("accepted");
    let worker = host.clone();
    let running = tokio::spawn(async move { worker.run().await });
    entered.notified().await;
    let stopping = host.clone();
    let shutdown = tokio::spawn(async move { stopping.shutdown().await });
    tokio::task::yield_now().await;

    assert!(!shutdown.is_finished());
    release.notify_one();
    shutdown.await.expect("shutdown joined");
    running.await.expect("worker joined").expect("worker exits");
    let state = store.state.lock().expect("state poisoned");
    assert!(matches!(
        state.as_ref(),
        Some((runtime::OperationState::Completed, _))
    ));
}

#[derive(Clone)]
struct AcceptStore {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

#[derive(Clone, Default)]
struct RunStore {
    state: Arc<Mutex<StoredState>>,
    idle: Arc<Notify>,
    completed: Arc<Notify>,
    /// Claims left to fail, and the error they fail with.
    failing_claims: Arc<AtomicUsize>,
    claim_error: Option<Error>,
    /// Heartbeats left to find their lease expired.
    lost_heartbeats: Arc<AtomicUsize>,
}

type StoredState = Option<(runtime::OperationState, Option<Outcome>)>;

impl RunStore {
    fn failing(claims: usize, error: Error) -> Self {
        Self {
            failing_claims: Arc::new(AtomicUsize::new(claims)),
            claim_error: Some(error),
            ..Self::default()
        }
    }

    fn enqueue(&self) {
        *self.state.lock().expect("state poisoned") =
            Some((runtime::OperationState::Accepted, None));
    }
}

impl Store for RunStore {
    async fn accept(&self, _prompt: &Prompt) -> Result<Accepted, Error> {
        *self.state.lock().expect("state poisoned") =
            Some((runtime::OperationState::Accepted, None));
        Ok(accepted())
    }

    async fn admit_with_tools(
        &self,
        prompt: &Prompt,
        _definitions: &[serde_json::Value],
    ) -> Result<runtime::Admitted, Error> {
        self.accept(prompt).await.map(Into::into)
    }

    async fn operation(&self, _id: OperationId) -> Result<Option<Operation>, Error> {
        Ok(self
            .state
            .lock()
            .expect("state poisoned")
            .clone()
            .map(|(state, outcome)| {
                let mut operation = accepted().operation;
                operation.state = state;
                operation.outcome = outcome;
                operation
            }))
    }

    async fn claim(&self, worker: &str, _ttl: Duration) -> Result<Option<Claimed>, Error> {
        let failing = self
            .failing_claims
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok();
        if let (true, Some(error)) = (failing, &self.claim_error) {
            return Err(error.clone());
        }
        let mut state = self.state.lock().expect("state poisoned");
        let Some((runtime::OperationState::Accepted, _)) = state.as_ref() else {
            self.idle.notify_one();
            return Ok(None);
        };
        *state = Some((runtime::OperationState::Running, None));
        let operation = accepted().operation;
        Ok(Some(Claimed {
            identity: None,
            operation: operation.clone(),
            thread: Thread::new("system", "hello"),
            tool_grant: None,
            lease: Lease {
                operation_id: operation.id,
                session_id: operation.session_id,
                worker: worker.to_owned(),
                token: 1,
            },
        }))
    }

    async fn claim_operation(
        &self,
        _id: OperationId,
        worker: &str,
        ttl: Duration,
    ) -> Result<Option<Claimed>, Error> {
        self.claim(worker, ttl).await
    }

    async fn record(&self, _lease: &Lease, event: &Event) -> Result<(), Error> {
        if let Event::RunFinished { outcome } = event {
            let state = match outcome {
                Outcome::Completed { .. } => runtime::OperationState::Completed,
                Outcome::Failed { .. } => runtime::OperationState::Failed,
                _ => {
                    return Err(Error::CorruptRecord {
                        message: "unknown outcome".to_owned(),
                    });
                }
            };
            *self.state.lock().expect("state poisoned") = Some((state, Some(outcome.clone())));
            self.completed.notify_one();
        }
        Ok(())
    }

    async fn heartbeat(&self, _lease: &Lease, _ttl: Duration) -> Result<(), Error> {
        let lost = self
            .lost_heartbeats
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            })
            .is_ok();
        if lost {
            return Err(Error::LeaseLost);
        }
        Ok(())
    }

    async fn compact(&self, _compaction: &Compaction) -> Result<(), Error> {
        Ok(())
    }
}

impl Store for AcceptStore {
    async fn accept(&self, _prompt: &Prompt) -> Result<Accepted, Error> {
        self.entered.notify_one();
        self.release.notified().await;
        Ok(accepted())
    }

    async fn admit_with_tools(
        &self,
        prompt: &Prompt,
        _definitions: &[serde_json::Value],
    ) -> Result<runtime::Admitted, Error> {
        self.accept(prompt).await.map(Into::into)
    }

    async fn operation(&self, _id: OperationId) -> Result<Option<Operation>, Error> {
        Ok(None)
    }

    async fn claim(&self, _worker: &str, _ttl: Duration) -> Result<Option<Claimed>, Error> {
        Ok(None)
    }

    async fn claim_operation(
        &self,
        _id: OperationId,
        _worker: &str,
        _ttl: Duration,
    ) -> Result<Option<Claimed>, Error> {
        Ok(None)
    }

    async fn record(&self, _lease: &Lease, _event: &Event) -> Result<(), Error> {
        Ok(())
    }

    async fn heartbeat(&self, _lease: &Lease, _ttl: Duration) -> Result<(), Error> {
        Ok(())
    }

    async fn compact(&self, _compaction: &Compaction) -> Result<(), Error> {
        Ok(())
    }
}

struct NoopModel;

struct FailureModel;

#[derive(Clone, Default)]
struct Logs(Arc<Mutex<Vec<u8>>>);

struct LogWriter(Logs);

impl Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Logs {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter(self.clone())
    }
}

#[derive(Default)]
struct Answer {
    called: Arc<Notify>,
    calls: Arc<AtomicUsize>,
}

#[derive(Default)]
struct BlockingModel {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

impl Model for BlockingModel {
    async fn generate(&self, _thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        self.entered.notify_one();
        self.release.notified().await;
        response("done")
    }
}

impl Model for Answer {
    async fn generate(&self, _thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.called.notify_one();
        response("done")
    }
}

impl Model for NoopModel {
    async fn generate(&self, _thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        response("unused")
    }
}

/// Outlasts the lease on its first call, then answers at once.
#[derive(Default)]
struct SlowThenAnswer {
    calls: AtomicUsize,
}

impl Model for SlowThenAnswer {
    async fn generate(&self, _thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_mins(1)).await;
        }
        response("done")
    }
}

/// Fails every summarization attempt of the first operation, then answers.
#[derive(Default)]
struct ThrottleThenAnswer {
    calls: Mutex<usize>,
}

impl Model for ThrottleThenAnswer {
    async fn generate(&self, _thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        let call = {
            let mut calls = self.calls.lock().expect("calls poisoned");
            *calls += 1;
            *calls
        };
        if call > 3 {
            return response("answered after the earlier lane failed");
        }
        ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            stop: StopReason::Error {
                code: "provider_unavailable".to_owned(),
                message: "Bedrock is unavailable".to_owned(),
            },
        }
    }
}

impl Model for FailureModel {
    async fn generate(&self, _thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: String::new(),
            tool_calls: Vec::new(),
            stop: StopReason::Error {
                code: "invalid_request".to_owned(),
                message: "provider rejected the request".to_owned(),
            },
        }
    }
}

fn response(text: &str) -> ModelResponse {
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: text.to_owned(),
        tool_calls: Vec::new(),
        stop: StopReason::EndTurn,
    }
}

fn accepted() -> Accepted {
    let operation = serde_json::from_value(serde_json::json!({
        "id": 1,
        "session_id": 1,
        "lane_id": 1,
        "state": "accepted",
        "outcome": null
    }))
    .expect("operation fixture");
    Accepted {
        operation,
        duplicate: false,
    }
}

fn prompt() -> Prompt {
    let address = Address::new("test", "conversation", "lane").expect("address");
    Prompt::new(address, "request-1", "system", "hello").expect("prompt")
}
