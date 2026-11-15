#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Policy behavior through the public durable runtime contract.
#![allow(clippy::too_many_lines)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use agent::{Message, Model, ModelResponse, Registry, StopReason, Thread};
use brainstone_runtime::{
    Address, Admitted, CompactionPolicy, Config, MemoryStore, OperationState, Prompt, Runtime,
    Store,
};
use tokio::sync::Notify;

#[derive(Clone)]
struct SteeringModel(Arc<SteeringState>);

struct SteeringState {
    calls: AtomicUsize,
    entered: Notify,
    release: Notify,
    second_input: Mutex<Vec<Message>>,
}

impl Model for SteeringModel {
    async fn generate(&self, thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        if self.0.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.0.entered.notify_one();
            self.0.release.notified().await;
            return response("stale answer");
        }
        *self.0.second_input.lock().expect("capture poisoned") = thread.messages().to_vec();
        response("corrected answer")
    }
}

#[tokio::test]
async fn an_active_lane_absorbs_durable_steering_before_it_finishes() {
    let runtime = Runtime::new(MemoryStore::default());
    let first = prompt("event-1", "do the first thing");
    let accepted = runtime.accept(&first).await.expect("accepted");
    let model = SteeringModel(Arc::new(SteeringState {
        calls: AtomicUsize::new(0),
        entered: Notify::new(),
        release: Notify::new(),
        second_input: Mutex::new(Vec::new()),
    }));
    let run = tokio::spawn({
        let runtime = runtime.clone();
        let model = model.clone();
        async move { runtime.run_next(&model, &Registry::default()).await }
    });
    model.0.entered.notified().await;

    let steered = runtime
        .admit_with_tools(&prompt("event-2", "change direction"), &Registry::default())
        .await
        .expect("steering is durable");
    assert!(matches!(
        steered,
        Admitted::Steering {
            duplicate: false,
            ..
        }
    ));
    model.0.release.notify_one();

    let operation = run.await.unwrap().unwrap().expect("operation");
    assert_eq!(operation.id, accepted.operation.id);
    assert_eq!(operation.state, OperationState::Completed);
    assert!(
        model
            .0
            .second_input
            .lock()
            .unwrap()
            .contains(&Message::User("change direction".to_owned()))
    );
    assert!(
        runtime
            .run_next(&response_model("unexpected"), &Registry::default())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn unconsumed_steering_survives_a_worker_crash() {
    let store = MemoryStore::default();
    let lease_ttl = std::time::Duration::from_secs(30);
    let first = Runtime::with_config(store.clone(), Config::new("crashed", lease_ttl).unwrap());
    first.accept(&prompt("crash-1", "original")).await.unwrap();
    let crashed = store
        .claim("crashed", lease_ttl)
        .await
        .unwrap()
        .expect("claimed before crash");
    first
        .admit_with_tools(
            &prompt("crash-2", "durable correction"),
            &Registry::default(),
        )
        .await
        .unwrap();
    drop(first);
    // The crashed worker's lease runs out, without racing the wall clock.
    store
        .heartbeat(&crashed.lease, std::time::Duration::ZERO)
        .await
        .unwrap();

    let capture = CaptureModel(Mutex::new(Vec::new()));
    Runtime::new(store)
        .run_next(&capture, &Registry::default())
        .await
        .unwrap();
    assert!(
        capture
            .0
            .lock()
            .unwrap()
            .contains(&Message::User("durable correction".to_owned()))
    );
}

struct CaptureModel(Mutex<Vec<Message>>);

impl Model for CaptureModel {
    async fn generate(&self, thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        *self.0.lock().unwrap() = thread.messages().to_vec();
        response("recovered")
    }
}

fn prompt(request_id: &str, text: &str) -> Prompt {
    let address = Address::new("test", "C123", "thread-42").unwrap();
    Prompt::new(address, request_id, "be useful", text).unwrap()
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

fn response_model(text: &str) -> impl Model {
    struct Answer(ModelResponse);
    impl Model for Answer {
        async fn generate(&self, _thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
            self.0.clone()
        }
    }
    Answer(response(text))
}

struct CompactingModel {
    calls: AtomicUsize,
    normal_input: Mutex<Vec<Message>>,
}

impl Model for CompactingModel {
    async fn generate(&self, thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            assert_eq!(
                thread.last(),
                &Message::User("summarize faithfully".to_owned())
            );
            return response("durable summary");
        }
        *self.normal_input.lock().unwrap() = thread.messages().to_vec();
        response("finished after compaction")
    }
}

#[tokio::test]
async fn automatic_compaction_replaces_the_prefix_in_the_durable_journal() {
    let store = MemoryStore::default();
    let policy = CompactionPolicy::new(1, "summarize faithfully").unwrap();
    let config = Config::new("worker", std::time::Duration::from_secs(30))
        .unwrap()
        .with_compaction(policy);
    let runtime = Runtime::with_config(store.clone(), config);
    runtime
        .accept(&prompt("compact-1", "a long conversation prefix"))
        .await
        .unwrap();
    let model = CompactingModel {
        calls: AtomicUsize::new(0),
        normal_input: Mutex::new(Vec::new()),
    };

    runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap();
    assert_eq!(
        *model.normal_input.lock().unwrap(),
        vec![Message::Summary("durable summary".to_owned())]
    );

    runtime
        .accept(&prompt("compact-2", "continue later"))
        .await
        .unwrap();
    let recovered = store
        .claim("replacement", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .expect("next operation");
    assert_eq!(
        recovered.thread.messages(),
        &[
            Message::Summary("durable summary".to_owned()),
            Message::Assistant(response("finished after compaction")),
            Message::User("continue later".to_owned()),
        ]
    );
}

/// Compaction runs before the run's first regular model call, so the head of
/// this script answers the summarization request. The last entry repeats, so a
/// script of one response answers every attempt.
struct Script {
    responses: Mutex<std::collections::VecDeque<ModelResponse>>,
    calls: AtomicUsize,
    efforts: Mutex<Vec<Option<agent::Effort>>>,
}

impl Script {
    fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            calls: AtomicUsize::new(0),
            efforts: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Model for Script {
    async fn generate(&self, thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.efforts.lock().unwrap().push(thread.profile().effort());
        let mut responses = self.responses.lock().unwrap();
        if responses.len() > 1 {
            return responses.pop_front().expect("scripted response");
        }
        responses.front().expect("scripted response").clone()
    }
}

fn throttled() -> ModelResponse {
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: String::new(),
        tool_calls: Vec::new(),
        stop: StopReason::Error {
            code: "rate_limited".to_owned(),
            message: "Bedrock throttled the request".to_owned(),
        },
    }
}

/// A stream that broke after the provider had already answered. The provider
/// seam retries until the stream opens, so this arrives having been tried once.
fn dropped() -> ModelResponse {
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: String::new(),
        tool_calls: Vec::new(),
        stop: StopReason::Error {
            code: "transport_error".to_owned(),
            message: "connection reset mid-stream".to_owned(),
        },
    }
}

fn truncated() -> ModelResponse {
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: "the summary stops half way th".to_owned(),
        tool_calls: Vec::new(),
        stop: StopReason::MaxTokens,
    }
}

fn compacting(
    store: MemoryStore,
    worker: &str,
    lease_ttl: std::time::Duration,
) -> Runtime<MemoryStore> {
    let policy = CompactionPolicy::new(1, "summarize faithfully").unwrap();
    let config = Config::new(worker, lease_ttl)
        .unwrap()
        .with_compaction(policy);
    Runtime::with_config(store, config)
}

#[tokio::test(start_paused = true)]
async fn a_broken_summary_stream_is_retried_before_the_turn_fails() {
    let store = MemoryStore::default();
    let runtime = compacting(store, "worker", std::time::Duration::from_secs(30));
    runtime
        .accept(&prompt("dropped-stream", "a long conversation prefix"))
        .await
        .unwrap();
    let model = Script::new(vec![dropped()]);

    let operation = runtime
        .run_next(&model, &Registry::default())
        .await
        .expect("a failed summary ends the turn, not the worker")
        .expect("the operation settles");

    assert_eq!(
        model.calls(),
        3,
        "a broken stream is retried before giving up"
    );
    assert_eq!(operation.state, OperationState::Failed);
    let Some(agent::Outcome::Failed { code, message }) = operation.outcome else {
        panic!("a throttled summary fails the operation");
    };
    assert_eq!(code, "transport_error", "the provider's own code survives");
    assert!(
        message.contains("compacting the thread") && message.contains("connection reset"),
        "the operator can tell which stage lost the stream: {message}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_broken_summary_stream_that_clears_finishes_the_turn() {
    let store = MemoryStore::default();
    let runtime = compacting(store, "worker", std::time::Duration::from_secs(30));
    runtime
        .accept(&prompt("recovers", "a long conversation prefix"))
        .await
        .unwrap();
    let model = Script::new(vec![
        dropped(),
        response("durable summary"),
        response("finished after compaction"),
    ]);

    let operation = runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .expect("the operation settles");

    assert_eq!(operation.state, OperationState::Completed);
    assert_eq!(
        operation.outcome,
        Some(agent::Outcome::Completed {
            text: "finished after compaction".to_owned(),
        })
    );
}

#[tokio::test]
async fn a_truncated_compaction_summary_fails_the_turn_under_a_stable_code() {
    let store = MemoryStore::default();
    let lease_ttl = std::time::Duration::from_secs(30);
    let runtime = compacting(store.clone(), "worker", lease_ttl);
    runtime
        .accept(&prompt("truncated", "a long conversation prefix"))
        .await
        .unwrap();
    let model = Script::new(vec![truncated()]);

    let operation = runtime
        .run_next(&model, &Registry::default())
        .await
        .expect("a truncated summary ends the turn, not the worker")
        .expect("the operation settles");

    assert_eq!(
        model.calls(),
        2,
        "the overflow buys one rewrite at less reasoning, and no more: an answered \
         request that overflowed again would overflow every time after that"
    );
    assert_eq!(operation.state, OperationState::Failed);
    let Some(agent::Outcome::Failed { code, message }) = operation.outcome else {
        panic!("a truncated summary fails the operation");
    };
    assert_eq!(code, "compaction_failed");
    assert!(
        message.contains("MaxTokens"),
        "the message separates an output-budget failure from a provider one: {message}"
    );
    // Finishing released the lease, so only the terminal state keeps the
    // replacement from claiming it.
    assert!(
        store
            .claim("replacement", lease_ttl)
            .await
            .unwrap()
            .is_none(),
        "a terminal operation is never re-claimed, so no crash loop"
    );
}

#[tokio::test]
async fn a_summary_that_fits_is_written_at_the_lane_s_own_effort() {
    let store = MemoryStore::default();
    let runtime = compacting(store, "worker", std::time::Duration::from_secs(30));
    let profile = agent::ExecutionProfile::new(None, Some(agent::Effort::XHigh), 25, 0).unwrap();
    runtime
        .accept(&prompt("effort", "a long conversation prefix").with_profile(profile))
        .await
        .unwrap();
    let model = Script::new(vec![response("durable summary"), response("done")]);

    runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .expect("the operation settles");

    assert_eq!(
        *model.efforts.lock().unwrap(),
        vec![Some(agent::Effort::XHigh), Some(agent::Effort::XHigh)],
        "a summary that fits is never quietly downgraded: the lane's effort stands"
    );
}

/// The summary shares one response budget with its own thinking, so a
/// reasoning-heavy lane can spend the whole budget before writing anything.
/// Dropping to `low` is worth one more call, but only once the budget has
/// actually been observed to overflow — never on speculation.
#[tokio::test]
async fn a_summary_that_overflows_its_budget_is_rewritten_with_less_reasoning() {
    let store = MemoryStore::default();
    let runtime = compacting(store, "worker", std::time::Duration::from_secs(30));
    let profile = agent::ExecutionProfile::new(None, Some(agent::Effort::XHigh), 25, 0).unwrap();
    runtime
        .accept(&prompt("overflow", "a long conversation prefix").with_profile(profile))
        .await
        .unwrap();
    let model = Script::new(vec![
        truncated(),
        response("durable summary"),
        response("finished after compaction"),
    ]);

    let operation = runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .expect("the operation settles");

    assert_eq!(operation.state, OperationState::Completed);
    assert_eq!(
        *model.efforts.lock().unwrap(),
        vec![
            Some(agent::Effort::XHigh),
            Some(agent::Effort::Low),
            Some(agent::Effort::XHigh)
        ],
        "the lane's effort is tried first, low is the fallback, and the run keeps its own"
    );
}

/// A blank summary is not a summary. It passes an emptiness check that only
/// looks for a zero-length string, and compaction is destructive: the rebuilt
/// thread starts from the summary, so accepting whitespace hides the whole
/// conversation from the model behind two spaces.
#[tokio::test]
async fn a_blank_summary_is_refused_rather_than_replacing_the_conversation() {
    let store = MemoryStore::default();
    let runtime = compacting(store.clone(), "worker", std::time::Duration::from_secs(30));
    runtime
        .accept(&prompt("blank", "a long conversation prefix"))
        .await
        .unwrap();
    let model = Script::new(vec![response("  \n\t ")]);

    let operation = runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .expect("the operation settles");

    assert_eq!(operation.state, OperationState::Failed);
    let Some(agent::Outcome::Failed { code, .. }) = operation.outcome else {
        panic!("a blank summary fails the operation");
    };
    assert_eq!(code, "compaction_failed");

    runtime
        .accept(&prompt("after-blank", "continue"))
        .await
        .unwrap();
    let recovered = store
        .claim("replacement", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .expect("next operation");
    assert!(
        recovered
            .thread
            .messages()
            .contains(&Message::User("a long conversation prefix".to_owned())),
        "the conversation survives a refused summary: {:?}",
        recovered.thread.messages()
    );
}

fn rejected_request() -> ModelResponse {
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: String::new(),
        tool_calls: Vec::new(),
        stop: StopReason::Error {
            code: "invalid_request".to_owned(),
            message: "request exceeds the model's input limit".to_owned(),
        },
    }
}

/// Both provider crates emit `invalid_request` for a request the provider
/// refused to look at. Sending the byte-identical thread again cannot change
/// that answer, so the attempt budget must not be spent on it.
#[tokio::test(start_paused = true)]
async fn a_permanently_rejected_summary_request_is_not_retried() {
    let store = MemoryStore::default();
    let runtime = compacting(store, "worker", std::time::Duration::from_secs(30));
    runtime
        .accept(&prompt("rejected", "a long conversation prefix"))
        .await
        .unwrap();
    let model = Script::new(vec![rejected_request()]);

    let operation = runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .expect("the operation settles");

    assert_eq!(model.calls(), 1, "a refused request is not sent again");
    assert_eq!(operation.state, OperationState::Failed);
    let Some(agent::Outcome::Failed { code, .. }) = operation.outcome else {
        panic!("a rejected summary fails the operation");
    };
    assert_eq!(code, "invalid_request");
}

struct BlockingScript {
    entered: Arc<Notify>,
    release: Arc<Notify>,
    calls: AtomicUsize,
}

impl Model for BlockingScript {
    async fn generate(&self, _thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.entered.notify_one();
            self.release.notified().await;
        }
        dropped()
    }
}

/// A failed summary appends no assistant message, so the turn budget cannot
/// bound this loop; steering absorbed by the failed settle sends it round
/// again. Draining the queue is correct — the user said something new — but
/// re-summarizing a thread that just refused to summarize is pure spend.
#[tokio::test]
async fn a_failed_summary_is_attempted_once_however_much_steering_arrives() {
    let store = MemoryStore::default();
    let runtime = compacting(store, "worker", std::time::Duration::from_secs(30));
    let address = Address::new("test", "C123", "thread-42").unwrap();
    runtime
        .accept(&prompt("steered", "a long conversation prefix"))
        .await
        .unwrap();
    let model = BlockingScript {
        entered: Arc::new(Notify::new()),
        release: Arc::new(Notify::new()),
        calls: AtomicUsize::new(0),
    };
    let entered = Arc::clone(&model.entered);
    let release = Arc::clone(&model.release);

    let running = tokio::spawn({
        let runtime = runtime.clone();
        async move {
            let model = model;
            let operation = runtime
                .run_next(&model, &Registry::default())
                .await
                .unwrap()
                .expect("the operation settles");
            (operation, model.calls.load(Ordering::SeqCst))
        }
    });
    entered.notified().await;
    for request in ["steer-1", "steer-2", "steer-3"] {
        let prompt = Prompt::new(address.clone(), request, "be useful", "and also this").unwrap();
        runtime
            .admit_with_tools(&prompt, &Registry::default())
            .await
            .unwrap();
    }
    release.notify_one();
    let (operation, calls) = running.await.unwrap();

    assert_eq!(operation.state, OperationState::Failed);
    assert_eq!(
        calls, 3,
        "one attempt budget for the run, not one per queued prompt"
    );
}

/// The provider seam already waits out a throttle properly — honouring
/// `retry-after` on the HTTP path, and 1 to 20 seconds of SDK backoff on
/// Bedrock — so one that reaches the runtime has either outlived that or needs
/// far longer than this loop waits. Repeating a thread-sized request into a
/// rate limiter adds load without changing the answer.
#[tokio::test(start_paused = true)]
async fn a_throttled_summary_is_not_retried_on_top_of_the_provider_backoff() {
    let store = MemoryStore::default();
    let runtime = compacting(store, "worker", std::time::Duration::from_secs(30));
    runtime
        .accept(&prompt("throttled", "a long conversation prefix"))
        .await
        .unwrap();
    let model = Script::new(vec![throttled()]);

    let operation = runtime
        .run_next(&model, &Registry::default())
        .await
        .expect("a throttled summary ends the turn, not the worker")
        .expect("the operation settles");

    assert_eq!(
        model.calls(),
        1,
        "sub-second backoff cannot clear a throttle, so asking again only adds load"
    );
    assert_eq!(operation.state, OperationState::Failed);
    let Some(agent::Outcome::Failed { code, .. }) = operation.outcome else {
        panic!("a throttled summary fails the operation");
    };
    assert_eq!(code, "rate_limited", "the provider's own code survives");
}

/// A provider with a small context window: any request past `limit` bytes of
/// messages is refused as too long, the way Bedrock refused a stalled restart.
struct Windowed {
    limit: usize,
    requests: Mutex<Vec<Vec<Message>>>,
}

impl Windowed {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<Vec<Message>> {
        self.requests.lock().unwrap().clone()
    }
}

impl Model for Windowed {
    async fn generate(&self, thread: &Thread, _tools: &[serde_json::Value]) -> ModelResponse {
        self.requests
            .lock()
            .unwrap()
            .push(thread.messages().to_vec());
        let size: usize = thread
            .messages()
            .iter()
            .map(|message| serde_json::to_string(message).unwrap().len())
            .sum();
        if size > self.limit {
            return overlong();
        }
        if thread.last() == &Message::User("summarize faithfully".to_owned()) {
            return response("durable summary");
        }
        response("finished after compaction")
    }
}

fn overlong() -> ModelResponse {
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: String::new(),
        tool_calls: Vec::new(),
        stop: StopReason::Error {
            code: agent::CODE_CONTEXT_WINDOW_EXCEEDED.to_owned(),
            message: "prompt is too long: 2432184 tokens > 1000000 maximum".to_owned(),
        },
    }
}

/// A user turn of about 1 KiB, named by its position.
fn turn(index: usize) -> String {
    format!("turn {index} {}", "é".repeat(500))
}

/// Four answered turns, written by a runtime that never compacts.
async fn history(store: &MemoryStore) {
    let config = Config::new("writer", std::time::Duration::from_secs(30)).unwrap();
    let writer = Runtime::with_config(store.clone(), config);
    for index in 0..4 {
        writer
            .accept(&prompt(&format!("turn-{index}"), &turn(index)))
            .await
            .unwrap();
        writer
            .run_next(&response_model("answered"), &Registry::default())
            .await
            .unwrap()
            .expect("the turn settles");
    }
}

/// The incident: a history too long for the provider to read, even to
/// summarize it. The summary is asked from the newest turns the window holds,
/// the turn finishes, and a replacement worker restores from that summary.
#[tokio::test]
async fn a_history_too_long_to_summarize_is_summarized_from_its_newest_turns() {
    let store = MemoryStore::default();
    history(&store).await;
    let runtime = compacting(store.clone(), "worker", std::time::Duration::from_secs(30));
    runtime.accept(&prompt("turn-4", &turn(4))).await.unwrap();
    let model = Windowed::new(3 * turn(0).len() + 1024);

    let operation = runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .expect("the operation settles");

    assert_eq!(
        operation.outcome,
        Some(agent::Outcome::Completed {
            text: "finished after compaction".to_owned(),
        })
    );
    let firsts: Vec<Message> = model.requests().iter().map(|r| r[0].clone()).collect();
    let expected = vec![
        Message::User(turn(0)),
        Message::User(turn(2)),
        Message::Summary("durable summary".to_owned()),
    ];
    assert_eq!(firsts, expected, "all five turns, then the newest three");

    runtime.accept(&prompt("turn-5", "continue")).await.unwrap();
    let recovered = store
        .claim("replacement", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .expect("next operation");
    assert_eq!(
        recovered.thread.messages(),
        &[
            Message::Summary("durable summary".to_owned()),
            Message::Assistant(response("finished after compaction")),
            Message::User("continue".to_owned()),
        ]
    );
}

/// A turn that alone outgrows the window cannot be summarized. It fails under
/// the stable code after one request, and the lane is not stuck: the next
/// prompt fits once that turn is left out, so the lane answers again.
#[tokio::test]
async fn a_turn_too_long_for_the_window_fails_without_sticking_the_lane() {
    let store = MemoryStore::default();
    let runtime = compacting(store, "worker", std::time::Duration::from_secs(30));
    let model = Windowed::new(4096);
    runtime
        .accept(&prompt("huge", &"x".repeat(8192)))
        .await
        .unwrap();

    let failed = runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .expect("the operation settles");

    assert_eq!(model.requests().len(), 1, "one turn has one rung");
    let Some(agent::Outcome::Failed { code, message }) = failed.outcome else {
        panic!("an unsummarizable turn fails the operation");
    };
    assert_eq!(code, agent::CODE_CONTEXT_WINDOW_EXCEEDED);
    assert!(message.contains("compacting the thread"), "{message}");

    runtime
        .accept(&prompt("next", "still there?"))
        .await
        .unwrap();
    let answered = runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .expect("the operation settles");
    assert_eq!(answered.state, OperationState::Completed);
}

/// Compaction by estimate never triggers here: only the provider's refusal
/// can tell the runtime the thread no longer fits.
fn compacting_on_refusal(store: MemoryStore) -> Runtime<MemoryStore> {
    let policy = CompactionPolicy::new(10_000_000, "summarize faithfully").unwrap();
    let config = Config::new("worker", std::time::Duration::from_secs(30))
        .unwrap()
        .with_compaction(policy);
    Runtime::with_config(store, config)
}

/// The estimate undercounts, so a regular request the
/// estimate let through is refused as too long. The runtime compacts and asks
/// again instead of failing every later turn the same way.
#[tokio::test]
async fn a_regular_request_refused_as_too_long_compacts_and_asks_again() {
    let store = MemoryStore::default();
    history(&store).await;
    let runtime = compacting_on_refusal(store.clone());
    runtime.accept(&prompt("turn-4", &turn(4))).await.unwrap();
    let model = Windowed::new(3 * turn(0).len() + 1024);

    let operation = runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .expect("the operation settles");

    assert_eq!(
        operation.outcome,
        Some(agent::Outcome::Completed {
            text: "finished after compaction".to_owned(),
        })
    );
    let lasts: Vec<Message> = model
        .requests()
        .iter()
        .map(|request| request.last().unwrap().clone())
        .collect();
    let summarize = Message::User("summarize faithfully".to_owned());
    let expected = vec![
        Message::User(turn(4)),
        summarize.clone(),
        summarize,
        Message::Summary("durable summary".to_owned()),
    ];
    assert_eq!(lasts, expected, "refused, two rungs, then the answer");

    runtime.accept(&prompt("turn-5", "continue")).await.unwrap();
    let recovered = store
        .claim("replacement", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .expect("next operation");
    assert_eq!(
        recovered.thread.messages()[0],
        Message::Summary("durable summary".to_owned()),
        "the refused request left no trace; the summary did"
    );
}

/// One prompt too long for the window has nothing to compact away. It fails
/// under the stable code after one request, never loops on summaries.
#[tokio::test]
async fn a_lone_prompt_too_long_for_the_window_fails_under_the_stable_code() {
    let runtime = compacting_on_refusal(MemoryStore::default());
    runtime
        .accept(&prompt("huge", &"x".repeat(8192)))
        .await
        .unwrap();
    let model = Windowed::new(4096);

    let operation = runtime
        .run_next(&model, &Registry::default())
        .await
        .unwrap()
        .expect("the operation settles");

    assert_eq!(model.requests().len(), 1);
    let Some(agent::Outcome::Failed { code, .. }) = operation.outcome else {
        panic!("the turn fails");
    };
    assert_eq!(code, agent::CODE_CONTEXT_WINDOW_EXCEEDED);
}
