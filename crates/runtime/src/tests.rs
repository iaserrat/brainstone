#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Runtime behavior table; long tests spell out crash and concurrency timing.
#![allow(clippy::too_many_lines)]

use crate::MemoryStore;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use agent::{
    Effort, Event, ExecutionProfile, Image, ImageMediaType, Message, Model, ModelResponse, Outcome,
    Registry, Sink, StopReason, Thread, Tool, ToolCall, ToolError, ToolRequest, UserImages,
};
use serde_json::{Value, json};
use tokio::sync::Notify;

use crate::{
    Address, Compaction, Config, DeliveryCheckpoint, DeliveryClaim, DeliveryStore, DeliveryTarget,
    InteractionRequest, InteractionStore, LaneStore, OperationState, Prompt, PromptContent,
    Runtime, Store, SubagentConfig, SubagentRequest,
};

fn image_prompt(
    address: Address,
    request_id: &str,
    system: &str,
    images: Vec<Image>,
) -> Result<Prompt, crate::Error> {
    let content = PromptContent {
        text: String::new(),
        images,
    };
    Prompt::with_content(address, request_id, system, content)
}

#[tokio::test]
async fn a_lane_can_stop_and_resume_following() {
    let store = MemoryStore::default();
    let address = Address::new("test", "channel", "thread").unwrap();
    let prompt = Prompt::new(address.clone(), "request", "system", "leave").unwrap();
    let operation = store.accept(&prompt).await.unwrap().operation.id;

    assert!(store.lane_is_followed(&address).await.unwrap());
    store.unfollow_lane(operation, "stop-call").await.unwrap();
    assert!(!store.lane_is_followed(&address).await.unwrap());
    store.follow_lane(&address, "mention").await.unwrap();
    assert!(store.lane_is_followed(&address).await.unwrap());
    store.unfollow_lane(operation, "stop-call").await.unwrap();
    assert!(store.lane_is_followed(&address).await.unwrap());
}

#[tokio::test]
async fn memory_store_replays_an_image_only_prompt_exactly() {
    let store = MemoryStore::default();
    let address = Address::new("test", "conversation", "lane").unwrap();
    let image = Image::new(ImageMediaType::Png, vec![1, 2, 3], 2, 3).unwrap();
    let prompt = image_prompt(address, "image", "system", vec![image.clone()]).unwrap();
    store.accept(&prompt).await.unwrap();
    let claimed = store
        .claim("worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        claimed.thread.messages(),
        &[Message::UserImages(
            UserImages::new("", vec![image]).unwrap()
        )]
    );
}

#[tokio::test]
async fn memory_store_replays_text_with_ordered_images() {
    let store = MemoryStore::default();
    let address = Address::new("test", "conversation", "text-images").unwrap();
    let first = Image::new(ImageMediaType::Png, vec![1], 1, 1).unwrap();
    let second = Image::new(ImageMediaType::Jpeg, vec![2], 1, 1).unwrap();
    let content = PromptContent {
        text: "inspect".to_owned(),
        images: vec![first.clone(), second.clone()],
    };
    let prompt = Prompt::with_content(address, "text-images", "system", content).unwrap();
    store.accept(&prompt).await.unwrap();
    let claimed = store
        .claim("worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        claimed.thread.messages(),
        &[Message::UserImages(
            UserImages::new("inspect", vec![first, second]).unwrap()
        )]
    );
}

#[tokio::test]
async fn memory_store_keeps_images_on_active_lane_steering() {
    let store = MemoryStore::default();
    let address = Address::new("test", "conversation", "steering").unwrap();
    let first = Prompt::new(address.clone(), "first", "system", "start").unwrap();
    store.accept(&first).await.unwrap();
    let claimed = store
        .claim("worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let image = Image::new(ImageMediaType::Png, vec![1, 2, 3], 2, 3).unwrap();
    let steer = image_prompt(address, "second", "system", vec![image.clone()]).unwrap();
    store.admit_with_tools(&steer, &[]).await.unwrap();
    assert_eq!(
        store.steering(&claimed.lease).await.unwrap(),
        vec![Message::UserImages(
            UserImages::new("", vec![image]).unwrap()
        )]
    );
}

#[test]
fn image_bytes_do_not_trigger_compaction_by_themselves() {
    let image = Image::new(ImageMediaType::Png, vec![7; 1_000_000], 10, 10).unwrap();
    let thread = Thread::restore(
        "system",
        vec![Message::UserImages(
            UserImages::new("inspect", vec![image]).unwrap(),
        )],
    )
    .unwrap();
    let policy = crate::CompactionPolicy::new(1, "summarize").unwrap();
    assert!(!policy.reached(&thread, &[]).unwrap());
}

#[tokio::test]
async fn steering_is_applied_one_durable_prompt_at_a_time() {
    let store = MemoryStore::default();
    let address = Address::new("test", "conversation", "serial-steering").unwrap();
    let first = Prompt::new(address.clone(), "first", "system", "start").unwrap();
    let accepted = store.accept(&first).await.unwrap();
    let claimed = store
        .claim_operation(
            accepted.operation.id,
            "worker",
            std::time::Duration::from_secs(30),
        )
        .await
        .unwrap()
        .unwrap();
    for (request, text) in [("second", "two"), ("third", "three")] {
        let prompt = Prompt::new(address.clone(), request, "system", text).unwrap();
        store.admit_with_tools(&prompt, &[]).await.unwrap();
    }

    assert_eq!(store.steering(&claimed.lease).await.unwrap().len(), 1);
    assert_eq!(store.steering(&claimed.lease).await.unwrap().len(), 1);
}

#[tokio::test]
async fn queued_steering_reaches_the_model_one_prompt_at_a_time() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store);
    let address = Address::new("test", "conversation", "queued-steering").unwrap();
    runtime
        .accept(&Prompt::new(address.clone(), "first", "system", "start").unwrap())
        .await
        .unwrap();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let model = QueuedSteeringModel {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
        seen: Arc::clone(&seen),
    };
    let runner = runtime.clone();
    let running = tokio::spawn(async move { runner.run_next(&model, &Registry::default()).await });
    entered.notified().await;
    for (request, text) in [("second", "two"), ("third", "three")] {
        let prompt = Prompt::new(address.clone(), request, "system", text).unwrap();
        runtime
            .admit_with_tools(&prompt, &Registry::default())
            .await
            .unwrap();
    }
    release.notify_one();
    running.await.unwrap().unwrap().unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    assert!(seen[1].contains(&Message::User("two".to_owned())));
    assert!(!seen[1].contains(&Message::User("three".to_owned())));
    assert!(seen[2].contains(&Message::User("three".to_owned())));
}

#[test]
fn compaction_bounds_replayed_image_bodies_but_not_the_current_turn() {
    let image = |size| Image::new(ImageMediaType::Png, vec![1; size], 1, 1).unwrap();
    let current = Thread::restore(
        "system",
        vec![Message::UserImages(
            UserImages::new("", vec![image(10 * 1024 * 1024)]).unwrap(),
        )],
    )
    .unwrap();
    let policy = crate::CompactionPolicy::new(100_000_000, "summarize").unwrap();
    assert!(!policy.reached(&current, &[]).unwrap());
    let historical = Thread::restore(
        "system",
        vec![
            Message::UserImages(UserImages::new("", vec![image(11 * 1024 * 1024)]).unwrap()),
            Message::UserImages(UserImages::new("", vec![image(10 * 1024 * 1024)]).unwrap()),
        ],
    )
    .unwrap();
    assert!(policy.reached(&historical, &[]).unwrap());
}

fn fetched(image: Image) -> Vec<Message> {
    vec![
        Message::User("read the screenshot".to_owned()),
        Message::Assistant(ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "call-1".to_owned(),
                name: "ReadThreadFile".to_owned(),
                args: serde_json::json!({"file_id": "F1"}),
            }],
            stop: StopReason::ToolUse,
        }),
        Message::ToolResults(vec![agent::ToolResult {
            id: "call-1".to_owned(),
            name: "ReadThreadFile".to_owned(),
            content: r#"{"outcome":"file_read"}"#.to_owned(),
            is_error: false,
            images: vec![image],
        }]),
    ]
}

// Base64 of 1 MB is 1.3M characters; sized as JSON text it would trip any
// budget. Sized as a 10x10 picture it costs one vision tile.
#[test]
fn a_tool_result_image_is_sized_by_its_pixels_not_its_bytes() {
    let image = Image::new(ImageMediaType::Png, vec![7; 1_000_000], 10, 10).unwrap();
    let thread = Thread::restore("system", fetched(image)).unwrap();
    let policy = crate::CompactionPolicy::new(10_000, "summarize").unwrap();

    assert!(!policy.reached(&thread, &[]).unwrap());
}

#[test]
fn tool_result_attachments_count_toward_the_replay_byte_bound() {
    let image = Image::new(ImageMediaType::Png, vec![1; 11 * 1024 * 1024], 1, 1).unwrap();
    let thread = Thread::restore("system", fetched(image)).unwrap();
    let policy = crate::CompactionPolicy::new(100_000_000, "summarize").unwrap();

    assert!(policy.reached(&thread, &[]).unwrap());
}

fn binaries(thread: &Thread) -> usize {
    thread
        .messages()
        .iter()
        .map(|message| match message {
            Message::UserImages(content) => content.images().len(),
            Message::ToolResults(results) => results.iter().map(|result| result.images.len()).sum(),
            _ => 0,
        })
        .sum()
}

// Three 5 MiB file reads replay past the provider's request cap. The summary
// is asked without them, so the lane compacts instead of failing every turn.
#[test]
fn a_summary_past_the_replay_bound_is_asked_without_binaries() {
    let big = || Image::new(ImageMediaType::Png, vec![1; 5 * 1024 * 1024], 1, 1).unwrap();
    let mut messages = fetched(big());
    messages.extend(fetched(big()).into_iter().skip(1));
    messages.extend(fetched(big()).into_iter().skip(1));
    let thread = Thread::restore("system", messages).unwrap();
    let policy = crate::CompactionPolicy::new(100_000_000, "summarize").unwrap();
    assert!(policy.reached(&thread, &[]).unwrap());

    let request = policy.summary_requests(&thread).next().unwrap();

    assert_eq!(binaries(&request), 0);
    assert_eq!(request.messages().len(), thread.messages().len() + 1);
    assert_eq!(request.last(), &Message::User("summarize".to_owned()));
    let Message::ToolResults(results) = &request.messages()[2] else {
        panic!("the tool batch keeps its place");
    };
    assert!(
        results[0].content.contains("omitted"),
        "{}",
        results[0].content
    );
}

#[test]
fn a_summary_within_the_replay_bound_keeps_its_binaries() {
    let small = Image::new(ImageMediaType::Png, vec![1; 1024], 1, 1).unwrap();
    let thread = Thread::restore("system", fetched(small)).unwrap();
    let policy = crate::CompactionPolicy::new(1, "summarize").unwrap();

    let first = policy.summary_requests(&thread).next().unwrap();
    assert_eq!(binaries(&first), 1);
}

fn said(text: &str) -> Message {
    Message::Assistant(ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: text.to_owned(),
        tool_calls: Vec::new(),
        stop: StopReason::EndTurn,
    })
}

/// `count` answered user turns, oldest first, each named by its position.
fn turns(count: usize) -> Vec<Message> {
    (0..count)
        .flat_map(|turn| [Message::User(format!("turn {turn}")), said("answered")])
        .collect()
}

fn first_turn(request: &Thread) -> &Message {
    &request.messages()[0]
}

// A history the provider refuses as too long must still be summarizable. Each
// rung after the first keeps the newest half of the turns, down to one, and
// every rung starts on a user turn and ends on the summarization prompt.
#[test]
fn each_summary_rung_keeps_the_newest_half_of_the_turns_down_to_one() {
    let thread = Thread::restore("system", turns(5)).unwrap();
    let policy = crate::CompactionPolicy::new(1, "summarize").unwrap();

    let firsts: Vec<Message> = policy
        .summary_requests(&thread)
        .map(|request| {
            assert_eq!(request.last(), &Message::User("summarize".to_owned()));
            first_turn(&request).clone()
        })
        .collect();

    let expected = [0, 2, 3, 4].map(|turn| Message::User(format!("turn {turn}")));
    assert_eq!(firsts, expected);
}

// Binaries go before any turn does: the rung after the first is the whole
// history as text, and only then does the history shrink.
#[test]
fn a_summary_drops_binaries_before_it_drops_turns() {
    let small = Image::new(ImageMediaType::Png, vec![1; 1024], 1, 1).unwrap();
    let mut messages = turns(1);
    messages.extend(fetched(small));
    let thread = Thread::restore("system", messages).unwrap();
    let policy = crate::CompactionPolicy::new(1, "summarize").unwrap();

    let requests: Vec<Thread> = policy.summary_requests(&thread).collect();

    let shape: Vec<(usize, usize)> = requests
        .iter()
        .map(|request| (binaries(request), request.messages().len()))
        .collect();
    assert_eq!(shape, [(1, 6), (0, 6), (0, 4)]);
}

// A result recorded before results were bounded replays at the bound, so the
// lane holding it can still be summarized.
#[test]
fn a_summary_bounds_a_tool_result_recorded_before_the_bound() {
    let mut messages = fetched(Image::new(ImageMediaType::Png, vec![1; 8], 1, 1).unwrap());
    let Message::ToolResults(results) = &mut messages[2] else {
        unreachable!("fetched ends on its tool batch");
    };
    results[0].images.clear();
    results[0].content = "x".repeat(8 * agent::MAX_TOOL_RESULT_BYTES);
    let thread = Thread::restore("system", messages).unwrap();
    let policy = crate::CompactionPolicy::new(1, "summarize").unwrap();

    let request = policy.summary_requests(&thread).next().unwrap();

    let Message::ToolResults(results) = &request.messages()[2] else {
        panic!("the tool batch keeps its place");
    };
    assert!(results[0].content.len() < agent::MAX_TOOL_RESULT_BYTES + 200);
    assert!(
        results[0]
            .content
            .contains(agent::CODE_TOOL_RESULT_TRUNCATED)
    );
}

#[test]
fn compaction_bounds_replayed_image_count_before_provider_limits() {
    let message = || {
        let images = (0..4)
            .map(|byte| Image::new(ImageMediaType::Png, vec![byte], 1, 1).unwrap())
            .collect();
        Message::UserImages(UserImages::new("", images).unwrap())
    };
    let thread = Thread::restore("system", (0..25).map(|_| message()).collect()).unwrap();
    let policy = crate::CompactionPolicy::new(100_000_000, "summarize").unwrap();

    assert!(policy.reached(&thread, &[]).unwrap());
}

#[test]
fn invalid_configuration_names_the_bad_field_without_a_false_reason() {
    let error = Config::new("worker", std::time::Duration::ZERO).expect_err("invalid lease");

    assert_eq!(error.code(), "invalid_input");
    assert_eq!(error.to_string(), "invalid lease_ttl");
}

#[test]
fn a_zero_presence_interval_is_invalid() {
    let error = DeliveryClaim::new("test", "surface", std::time::Duration::from_secs(30), 32)
        .unwrap()
        .with_presence(std::time::Duration::ZERO)
        .expect_err("zero presence");
    assert_eq!(error.code(), "invalid_input");
    assert_eq!(error.to_string(), "invalid delivery_presence");
}

#[test]
fn unsafe_postgres_settings_are_named_in_the_durability_error() {
    let error = crate::postgres_durability::require_on("fsync", "off").expect_err("unsafe");

    assert_eq!(error.code(), "durability_not_guaranteed");
    assert_eq!(error.to_string(), "fsync is off, expected on");
}

struct Scripted(Mutex<Option<ModelResponse>>);

struct Capturing {
    seen: Mutex<Vec<Message>>,
}

struct CapturingProfile {
    seen: Mutex<Option<ExecutionProfile>>,
}

impl Capturing {
    fn new() -> Self {
        Self {
            seen: Mutex::new(Vec::new()),
        }
    }
}

impl Model for Capturing {
    async fn generate(&self, thread: &Thread, _tools: &[Value]) -> ModelResponse {
        self.seen
            .lock()
            .expect("capture poisoned")
            .clone_from(&thread.messages().to_vec());
        answer("second answer")
            .0
            .lock()
            .expect("script poisoned")
            .take()
            .expect("answer")
    }
}

impl Model for CapturingProfile {
    async fn generate(&self, thread: &Thread, _tools: &[Value]) -> ModelResponse {
        self.seen
            .lock()
            .expect("capture poisoned")
            .replace(thread.profile().clone());
        answer("done")
            .0
            .lock()
            .expect("script poisoned")
            .take()
            .expect("answer")
    }
}

impl Model for Scripted {
    async fn generate(&self, _thread: &Thread, _tools: &[Value]) -> ModelResponse {
        self.0
            .lock()
            .expect("script poisoned")
            .take()
            .expect("one call")
    }
}

fn answer(text: &str) -> Scripted {
    Scripted(Mutex::new(Some(ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: text.to_owned(),
        tool_calls: Vec::new(),
        stop: StopReason::EndTurn,
    })))
}

struct Blocking {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

struct QueuedSteeringModel {
    entered: Arc<Notify>,
    release: Arc<Notify>,
    seen: Arc<Mutex<Vec<Vec<Message>>>>,
}

struct ToolModel;

struct StreamingModel(AtomicBool);

impl Model for StreamingModel {
    async fn generate(&self, _thread: &Thread, _tools: &[Value]) -> ModelResponse {
        panic!("the runtime must use the streaming model path")
    }

    async fn generate_stream<S: Sink + Send>(
        &self,
        _thread: &Thread,
        _tools: &[Value],
        sink: &mut S,
    ) -> Result<ModelResponse, S::Error> {
        sink.emit(Event::ModelDelta {
            delta: "live".to_owned(),
        })
        .await?;
        self.0.store(true, Ordering::SeqCst);
        Ok(answer("done")
            .0
            .lock()
            .expect("script")
            .take()
            .expect("answer"))
    }
}

impl Model for ToolModel {
    async fn generate(&self, _thread: &Thread, _tools: &[Value]) -> ModelResponse {
        ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "provider-call-1".to_owned(),
                name: "retry".to_owned(),
                args: json!({"value": "café"}),
            }],
            stop: StopReason::ToolUse,
        }
    }
}

struct RetryTool {
    keys: Mutex<Vec<Option<String>>>,
    entered: Notify,
    never: Notify,
}

impl RetryTool {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            keys: Mutex::new(Vec::new()),
            entered: Notify::new(),
            never: Notify::new(),
        })
    }
}

impl Tool for RetryTool {
    fn name(&self) -> &'static str {
        "retry"
    }

    fn definition(&self) -> Value {
        json!({"name": "retry", "description": "test", "input_schema": {"type": "object"}})
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<agent::ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async move {
            let call = {
                let mut keys = self.keys.lock().expect("keys poisoned");
                keys.push(request.idempotency_key);
                keys.len()
            };
            if call == 1 {
                self.entered.notify_one();
                self.never.notified().await;
            }
            Ok(request.args.to_string().into())
        })
    }
}

impl Model for Blocking {
    async fn generate(&self, _thread: &Thread, _tools: &[Value]) -> ModelResponse {
        self.entered.notify_one();
        self.release.notified().await;
        answer("first done")
            .0
            .lock()
            .expect("script poisoned")
            .take()
            .expect("answer")
    }
}

impl Model for QueuedSteeringModel {
    async fn generate(&self, thread: &Thread, _tools: &[Value]) -> ModelResponse {
        let first = {
            let mut seen = self.seen.lock().expect("capture poisoned");
            seen.push(thread.messages().to_vec());
            seen.len() == 1
        };
        if first {
            self.entered.notify_one();
            self.release.notified().await;
        }
        answer("done")
            .0
            .lock()
            .expect("script poisoned")
            .take()
            .expect("answer")
    }
}

fn prompt() -> Prompt {
    let address = Address::new("test", "C123", "thread-42").expect("valid address");
    Prompt::new(address, "event-7", "be useful", "héllo ☕").expect("valid prompt")
}

#[tokio::test]
async fn an_accepted_prompt_survives_runtime_replacement() {
    let store = MemoryStore::default();
    let first = Runtime::new(store.clone());
    let accepted = first.accept(&prompt()).await.expect("accepted");
    drop(first);

    let reopened = Runtime::new(store);
    let restored = reopened
        .operation(accepted.operation.id)
        .await
        .expect("read succeeds")
        .expect("operation survives");

    assert_eq!(restored.state, OperationState::Accepted);
}

#[tokio::test]
async fn execution_profile_survives_runtime_replacement() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let profile = ExecutionProfile::new(Some("claude-sonnet-5"), Some(Effort::Low), 7, 2).unwrap();
    runtime
        .accept(&prompt().with_profile(profile.clone()))
        .await
        .expect("accepted");
    drop(runtime);
    let model = CapturingProfile {
        seen: Mutex::new(None),
    };

    Runtime::new(store)
        .run_next(&model, &Registry::default())
        .await
        .expect("run");

    assert_eq!(model.seen.into_inner().unwrap(), Some(profile));
}

#[tokio::test]
async fn an_idle_lane_adopts_the_next_operations_turn_budget() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let address = Address::new("test", "channel", "reconfigured").unwrap();
    let first_profile = ExecutionProfile::new(None, None, 25, 0).unwrap();
    let first = Prompt::new(address.clone(), "first", "system", "one")
        .unwrap()
        .with_profile(first_profile);
    runtime
        .admit_with_tools(&first, &Registry::default())
        .await
        .unwrap();
    runtime
        .run_next(&answer("done"), &Registry::default())
        .await
        .unwrap();
    let next_profile = ExecutionProfile::new(None, None, 100, 0).unwrap();
    let next = Prompt::new(address, "next", "system", "two")
        .unwrap()
        .with_profile(next_profile);

    runtime
        .admit_with_tools(&next, &Registry::default())
        .await
        .unwrap();
    let claimed = store
        .claim("worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(claimed.thread.profile().max_turns(), Some(100));
}

#[tokio::test]
async fn surface_admission_keeps_the_existing_lane_system_prompt() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let address = Address::new("test", "channel", "stored-system").unwrap();
    let first = Prompt::new(address.clone(), "first", "original system", "one").unwrap();
    runtime
        .admit_with_tools(&first, &Registry::default())
        .await
        .unwrap();
    runtime
        .run_next(&answer("done"), &Registry::default())
        .await
        .unwrap();
    let next = Prompt::new(address, "next", "replacement system", "two").unwrap();

    runtime
        .admit_with_tools(&next, &Registry::default())
        .await
        .unwrap();
    let claimed = store
        .claim("worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(claimed.thread.system, "original system");
}

#[tokio::test]
async fn surface_admission_uses_the_supplied_system_prompt_for_a_new_lane() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let address = Address::new("test", "channel", "new-system").unwrap();
    let prompt = Prompt::new(address, "first", "deployment system", "one").unwrap();

    let admitted = runtime
        .admit_with_tools(&prompt, &Registry::default())
        .await
        .unwrap();
    let claimed = store
        .claim_operation(
            admitted.operation().id,
            "worker",
            std::time::Duration::from_secs(30),
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(claimed.thread.system, "deployment system");
}

#[tokio::test]
async fn surface_admission_steers_after_a_deployment_configuration_change() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let address = Address::new("test", "channel", "active-system").unwrap();
    let original = ExecutionProfile::new(None, None, 25, 0).unwrap();
    let first = Prompt::new(address.clone(), "first", "original system", "one")
        .unwrap()
        .with_profile(original);
    let admitted = runtime
        .admit_with_tools(&first, &Registry::default())
        .await
        .unwrap();
    let claimed = store
        .claim_operation(
            admitted.operation().id,
            "worker",
            std::time::Duration::from_secs(30),
        )
        .await
        .unwrap()
        .unwrap();
    let replacement = ExecutionProfile::new(None, None, 100, 0).unwrap();
    let next = Prompt::new(address, "next", "replacement system", "two")
        .unwrap()
        .with_profile(replacement);

    let steered = runtime
        .admit_with_tools(&next, &Registry::default())
        .await
        .unwrap();

    assert!(matches!(steered, crate::Admitted::Steering { .. }));
    assert_eq!(claimed.thread.system, "original system");
    assert_eq!(claimed.thread.profile().max_turns(), Some(25));
    assert_eq!(
        store.steering(&claimed.lease).await.unwrap(),
        vec![Message::User("two".to_owned())]
    );
}

#[tokio::test]
async fn surface_redelivery_ignores_changed_deployment_configuration() {
    let runtime = Runtime::new(MemoryStore::default());
    let address = Address::new("test", "channel", "duplicate-system").unwrap();
    let original = ExecutionProfile::new(None, None, 25, 0).unwrap();
    let first = Prompt::new(address.clone(), "request", "original system", "one")
        .unwrap()
        .with_profile(original);
    runtime
        .admit_with_tools(&first, &Registry::default())
        .await
        .unwrap();
    let profile = ExecutionProfile::new(None, None, 100, 0).unwrap();
    let replay = Prompt::new(address, "request", "replacement system", "one")
        .unwrap()
        .with_profile(profile);

    let admitted = runtime
        .admit_with_tools(&replay, &Registry::default())
        .await
        .unwrap();

    assert!(matches!(
        admitted,
        crate::Admitted::Operation(crate::Accepted {
            duplicate: true,
            ..
        })
    ));
}

#[tokio::test]
async fn low_level_accept_keeps_existing_lane_configuration_strict() {
    let store = MemoryStore::default();
    let address = Address::new("test", "strict", "lane").unwrap();
    let first = Prompt::new(address.clone(), "first", "system", "one").unwrap();
    store.accept(&first).await.unwrap();
    let profile = ExecutionProfile::new(None, None, 7, 0).unwrap();
    let changed = Prompt::new(address, "first", "system", "one")
        .unwrap()
        .with_profile(profile);

    assert_eq!(
        store.accept(&changed).await.unwrap_err(),
        crate::Error::LaneConfigurationConflict
    );
}

#[test]
fn stored_subagent_records_reject_unknown_fields() {
    let value = json!({
        "parent": 1,
        "key": "spawn",
        "task": "work",
        "config": {
            "name": "worker",
            "system": "system",
            "profile": {"model":null,"effort":null,"max_turns":25,"depth":1,
                "allowed_agents":null},
            "tools": [],
            "background": false
        },
        "future_field": true
    });

    assert!(serde_json::from_value::<SubagentRequest>(value).is_err());
}

#[tokio::test]
async fn subagent_spawn_is_idempotent_and_uses_an_independent_session() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let parent = runtime.accept(&prompt()).await.expect("parent accepted");
    let parent_claim = store
        .claim("parent-worker", std::time::Duration::from_secs(30))
        .await
        .expect("claim")
        .expect("parent claim");
    let profile = ExecutionProfile::new(None, Some(Effort::Medium), 8, 1).unwrap();
    let config = SubagentConfig::new("reviewer", "review system", profile).expect("config");
    let request = SubagentRequest::new(parent.operation.id, "spawn-key", "review task", config)
        .expect("request");

    let first = runtime.spawn_subagent(&request).await.expect("spawned");
    let retry = runtime.spawn_subagent(&request).await.expect("same child");
    let child_claim = store
        .claim("child-worker", std::time::Duration::from_secs(30))
        .await
        .expect("claim")
        .expect("child claim");

    assert_eq!(first.agent.id, retry.agent.id);
    assert!(!first.duplicate);
    assert!(retry.duplicate);
    assert_ne!(
        first.agent.operation.session_id,
        parent_claim.operation.session_id
    );
    assert_eq!(child_claim.operation.id, first.agent.operation.id);
    assert_eq!(child_claim.thread.system, "review system");
    assert_eq!(child_claim.thread.profile().depth(), 1);
}

#[tokio::test]
async fn subagent_follow_up_resumes_its_durable_context() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let parent = runtime.accept(&prompt()).await.unwrap();
    store
        .claim("parent", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let config =
        SubagentConfig::new("reviewer", "review system", ExecutionProfile::default()).unwrap();
    let request =
        SubagentRequest::new(parent.operation.id, "spawn-key", "first task", config).unwrap();
    let child = runtime.spawn_subagent(&request).await.unwrap().agent;
    runtime
        .run_next(&answer("first result"), &Registry::default())
        .await
        .unwrap();

    let resumed = runtime
        .message_subagent(parent.operation.id, child.id, "follow-up", "second task")
        .await
        .unwrap();
    let claimed = store
        .claim("worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .expect("follow-up claim");

    assert_eq!(resumed.operation.session_id, child.operation.session_id);
    assert_eq!(
        claimed.thread.messages()[0],
        Message::User("first task".to_owned())
    );
    assert_eq!(
        claimed.thread.messages()[2],
        Message::User("second task".to_owned())
    );
}

#[tokio::test]
async fn stopping_a_subagent_fences_its_active_writer() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let parent = runtime.accept(&prompt()).await.unwrap();
    store
        .claim("parent", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let config =
        SubagentConfig::new("reviewer", "review system", ExecutionProfile::default()).unwrap();
    let request = SubagentRequest::new(parent.operation.id, "spawn-key", "task", config).unwrap();
    let child = runtime.spawn_subagent(&request).await.unwrap().agent;
    let lease = store
        .claim("worker", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .expect("child claim")
        .lease;

    let stopped = runtime
        .stop_subagent(parent.operation.id, child.id)
        .await
        .unwrap();

    assert_eq!(stopped.operation.state, OperationState::Failed);
    assert!(
        matches!(stopped.operation.outcome, Some(Outcome::Failed { ref code, .. }) if code == "agent_cancelled")
    );
    assert_eq!(
        store.record(&lease, &Event::RunStarted).await.unwrap_err(),
        crate::Error::LeaseLost
    );
}

#[tokio::test]
async fn unrelated_sessions_cannot_control_a_subagent() {
    let runtime = Runtime::new(MemoryStore::default());
    let parent = runtime.accept(&prompt()).await.unwrap();
    let config =
        SubagentConfig::new("reviewer", "review system", ExecutionProfile::default()).unwrap();
    let request = SubagentRequest::new(parent.operation.id, "spawn-key", "task", config).unwrap();
    let child = runtime.spawn_subagent(&request).await.unwrap().agent;
    let address = Address::new("test", "OTHER", "thread").unwrap();
    let outsider = runtime
        .accept(&Prompt::new(address, "other", "system", "prompt").unwrap())
        .await
        .unwrap();

    let error = runtime
        .subagent(outsider.operation.id, child.id)
        .await
        .unwrap_err();

    assert_eq!(error, crate::Error::InvalidInput { field: "agent_id" });
}

#[tokio::test]
async fn accepting_the_same_surface_request_is_idempotent() {
    let runtime = Runtime::new(MemoryStore::default());

    let first = runtime.accept(&prompt()).await.expect("first delivery");
    let retry = runtime.accept(&prompt()).await.expect("delivery retry");

    assert_eq!(retry.operation.id, first.operation.id);
    assert!(!first.duplicate);
    assert!(retry.duplicate);
}

#[tokio::test]
async fn idempotency_uses_text_before_admission_enrichment() {
    let store = MemoryStore::default();
    let raw = prompt();
    let first = raw
        .clone()
        .with_text("<memories>first</memories>\n\nhéllo ☕")
        .unwrap();
    let retry = raw
        .with_text("<memories>changed</memories>\n\nhéllo ☕")
        .unwrap();

    let accepted = store.accept(&first).await.expect("accepted");
    let duplicate = store.accept(&retry).await.expect("same original input");

    assert!(duplicate.duplicate);
    assert_eq!(duplicate.operation.id, accepted.operation.id);
}

#[tokio::test]
async fn named_execution_waits_for_an_earlier_operation_in_its_lane() {
    let runtime = Runtime::new(MemoryStore::default());
    let first = runtime.accept(&prompt()).await.expect("first");
    let mut second_prompt = prompt();
    second_prompt.request_id = "event-8".to_owned();
    let second = runtime.accept(&second_prompt).await.expect("second");

    assert!(
        runtime
            .run_operation(
                second.operation.id,
                &answer("blocked"),
                &Registry::default(),
            )
            .await
            .unwrap()
            .is_none()
    );
    runtime
        .run_operation(first.operation.id, &answer("first"), &Registry::default())
        .await
        .unwrap();
    let completed = runtime
        .run_operation(second.operation.id, &answer("second"), &Registry::default())
        .await
        .unwrap();
    assert!(completed.is_some());
}

#[tokio::test]
async fn an_interaction_request_and_answer_survive_reopening() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let prompt = prompt().with_delivery(
        DeliveryTarget::new("test", json!({"conversation": "C123"})).expect("target"),
    );
    let accepted = runtime.accept(&prompt).await.expect("accepted");
    let request = InteractionRequest::new(
        accepted.operation.id,
        "operation/1/call/1/input/confirm",
        json!({"message": "Proceed?"}),
    )
    .expect("request");
    let opened = store.open_interaction(&request).await.expect("opened");
    store
        .mark_interaction_delivered(&request.id, "171.1")
        .await
        .expect("delivered");
    store
        .answer_interaction(&request.id, &json!({"action": "accept"}))
        .await
        .expect("answered");

    let reopened = store.open_interaction(&request).await.expect("reopened");
    assert_eq!(opened.client_message_id, reopened.client_message_id);
    assert_eq!(reopened.external_id.as_deref(), Some("171.1"));
    assert_eq!(reopened.response, Some(json!({"action": "accept"})));
}

#[tokio::test]
async fn a_request_id_cannot_be_reused_for_different_input() {
    let runtime = Runtime::new(MemoryStore::default());
    let first = prompt();
    let changed = Prompt::new(first.address.clone(), "event-7", "be useful", "different")
        .expect("valid prompt");

    runtime.accept(&first).await.expect("first delivery");
    let error = runtime
        .accept(&changed)
        .await
        .expect_err("identity conflict");

    assert_eq!(error.code(), "idempotency_conflict");
}

#[tokio::test]
async fn a_request_id_cannot_change_its_delivery_target() {
    let runtime = Runtime::new(MemoryStore::default());
    let first = prompt()
        .with_delivery(DeliveryTarget::new("test", json!({"conversation": "C123"})).unwrap());
    let changed = prompt()
        .with_delivery(DeliveryTarget::new("test", json!({"conversation": "C999"})).unwrap());

    runtime.accept(&first).await.expect("first delivery");
    let error = runtime
        .accept(&changed)
        .await
        .expect_err("identity conflict");

    assert_eq!(error.code(), "idempotency_conflict");
}

#[test]
fn a_delivery_target_requires_an_adapter_name() {
    let error = DeliveryTarget::new("", json!({})).expect_err("invalid target");

    assert_eq!(error.code(), "invalid_input");
    assert_eq!(error.to_string(), "invalid delivery_adapter");
}

#[tokio::test]
async fn one_surface_conversation_has_one_session_and_distinct_lanes() {
    let runtime = Runtime::new(MemoryStore::default());
    let address_a = Address::new("test", "C123", "thread-a").expect("valid");
    let address_b = Address::new("test", "C123", "thread-b").expect("valid");
    let prompt_a = Prompt::new(address_a, "event-a", "system", "one").expect("valid");
    let prompt_b = Prompt::new(address_b, "event-b", "system", "two").expect("valid");

    let first = runtime.accept(&prompt_a).await.expect("accepted");
    let second = runtime.accept(&prompt_b).await.expect("accepted");

    assert_eq!(first.operation.session_id, second.operation.session_id);
    assert_ne!(first.operation.lane_id, second.operation.lane_id);
}

#[tokio::test]
async fn a_lane_cannot_silently_change_its_system_prompt() {
    let runtime = Runtime::new(MemoryStore::default());
    let first = prompt();
    let changed =
        Prompt::new(first.address.clone(), "event-8", "new system", "next").expect("valid prompt");

    runtime.accept(&first).await.expect("lane created");
    let error = runtime
        .accept(&changed)
        .await
        .expect_err("configuration conflict");

    assert_eq!(error.code(), "lane_configuration_conflict");
}

#[tokio::test]
async fn an_accepted_prompt_runs_to_a_durable_outcome() {
    let runtime = Runtime::new(MemoryStore::default());
    let accepted = runtime.accept(&prompt()).await.expect("accepted");

    let finished = runtime
        .run_next(&answer("done ✅"), &Registry::default())
        .await
        .expect("run succeeds")
        .expect("work existed");

    assert_eq!(finished.id, accepted.operation.id);
    assert_eq!(finished.state, OperationState::Completed);
    assert_eq!(
        finished.outcome,
        Some(Outcome::Completed {
            text: "done ✅".to_owned()
        })
    );
}

#[tokio::test]
async fn the_runtime_records_the_models_streaming_path() {
    let runtime = Runtime::new(MemoryStore::default());
    runtime.accept(&prompt()).await.expect("accepted");
    let model = StreamingModel(AtomicBool::new(false));

    runtime
        .run_next(&model, &Registry::default())
        .await
        .expect("run succeeds");

    assert!(model.0.load(Ordering::SeqCst));
}

#[tokio::test]
async fn delivery_replays_durable_events_and_advances_one_fenced_cursor() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let prompt = prompt()
        .with_delivery(DeliveryTarget::new("test", json!({"conversation": "C123"})).unwrap());
    runtime.accept(&prompt).await.expect("accepted");
    runtime
        .run_next(
            &StreamingModel(AtomicBool::new(false)),
            &Registry::default(),
        )
        .await
        .expect("run succeeds");

    let wrong = DeliveryClaim::new(
        "http",
        "surface-http",
        std::time::Duration::from_secs(30),
        32,
    )
    .unwrap();
    assert!(
        store
            .claim_delivery(&wrong)
            .await
            .expect("claim succeeds")
            .is_none()
    );

    let batch = store
        .claim_delivery(
            &DeliveryClaim::new("test", "surface-1", std::time::Duration::from_secs(30), 32)
                .unwrap(),
        )
        .await
        .expect("claim succeeds")
        .expect("events are pending");
    assert_eq!(batch.target.data["conversation"], "C123");
    assert!(matches!(batch.events[1].event, Event::ModelDelta { .. }));
    assert_eq!(
        store
            .renew_delivery(&batch.lease, std::time::Duration::MAX)
            .await
            .expect_err("an unrepresentable lease duration is invalid input")
            .code(),
        "invalid_input"
    );
    assert!(
        store
            .claim_delivery(
                &DeliveryClaim::new("test", "surface-2", std::time::Duration::from_secs(30), 32)
                    .unwrap()
            )
            .await
            .expect("second claim")
            .is_none()
    );

    let through = batch.events.last().expect("events").sequence;
    let overflow = DeliveryCheckpoint::new(batch.lease.clone(), i64::MAX, None);
    assert_eq!(
        store
            .checkpoint_delivery(&overflow)
            .await
            .expect_err("the cursor must not overflow")
            .code(),
        "invalid_delivery_checkpoint"
    );
    let invalid = DeliveryCheckpoint::new(batch.lease.clone(), through + 1, None);
    assert_eq!(
        store
            .checkpoint_delivery(&invalid)
            .await
            .expect_err("cursor beyond journal")
            .code(),
        "invalid_delivery_checkpoint"
    );
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(
            batch.lease,
            through,
            Some("test-message-1".to_owned()),
        ))
        .await
        .expect("checkpoint succeeds");
    assert!(
        store
            .claim_delivery(
                &DeliveryClaim::new("test", "surface-2", std::time::Duration::from_secs(30), 32)
                    .unwrap()
            )
            .await
            .expect("claim succeeds")
            .is_none()
    );
}

#[tokio::test]
async fn a_running_operation_stays_presentable_after_its_journal_is_caught_up() {
    let store = MemoryStore::default();
    let prompt = prompt()
        .with_delivery(DeliveryTarget::new("test", json!({"conversation": "C123"})).unwrap());
    store.accept(&prompt).await.expect("accepted");
    let claimed = store
        .claim("runner", std::time::Duration::from_secs(30))
        .await
        .expect("claim")
        .expect("pending");
    store
        .record(&claimed.lease, &Event::RunStarted)
        .await
        .expect("started");

    let presence = std::time::Duration::from_millis(20);
    let claim = DeliveryClaim::new("test", "surface-1", std::time::Duration::from_secs(30), 32)
        .unwrap()
        .with_presence(presence)
        .unwrap();
    let started = store
        .claim_delivery(&claim)
        .await
        .expect("claim")
        .expect("RunStarted is pending");
    assert_eq!(started.events.len(), 1);
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(
            started.lease,
            started.events[0].sequence,
            None,
        ))
        .await
        .expect("caught up");

    assert!(
        store.claim_delivery(&claim).await.expect("claim").is_none(),
        "a just-presented running operation is not owed another claim"
    );

    tokio::time::sleep(presence + std::time::Duration::from_millis(5)).await;
    let live = store
        .claim_delivery(&claim)
        .await
        .expect("claim")
        .expect("a still-running operation must stay presentable");
    assert!(
        live.events.is_empty(),
        "the journal is caught up; this claim is presence, not replay"
    );
    store
        .checkpoint_delivery(&DeliveryCheckpoint::presented(live.lease))
        .await
        .expect("presence recorded");

    store
        .record(
            &claimed.lease,
            &Event::RunFinished {
                outcome: Outcome::Completed {
                    text: "done".to_owned(),
                },
            },
        )
        .await
        .expect("finished");
    let finished = store
        .claim_delivery(&claim)
        .await
        .expect("claim")
        .expect("the outcome is still owed");
    assert!(
        matches!(
            finished.events.last().map(|event| &event.event),
            Some(Event::RunFinished { .. })
        ),
        "finishing must not be swallowed by a presence claim"
    );
    let through = finished.events.last().expect("events").sequence;
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(finished.lease, through, None))
        .await
        .expect("terminal checkpoint");
    assert!(
        store.claim_delivery(&claim).await.expect("claim").is_none(),
        "a finished caught-up operation is fully delivered"
    );
}

#[tokio::test]
async fn accepted_work_is_presentable_while_it_waits_for_the_session_writer() {
    let store = MemoryStore::default();
    let prompt = prompt()
        .with_delivery(DeliveryTarget::new("test", json!({"conversation": "C123"})).unwrap());
    store.accept(&prompt).await.expect("accepted");
    let claim = DeliveryClaim::new("test", "surface", std::time::Duration::from_secs(30), 32)
        .unwrap()
        .with_presence(std::time::Duration::from_hours(1))
        .unwrap();

    let waiting = store
        .claim_delivery(&claim)
        .await
        .expect("claim")
        .expect("accepted work must stay visible while queued");

    assert!(waiting.events.is_empty(), "the run has not started yet");
}

#[tokio::test]
async fn a_new_event_preempts_an_empty_presence_lease() {
    let store = MemoryStore::default();
    let target = DeliveryTarget::new("test", json!({"conversation": "C123"})).unwrap();
    store.accept(&prompt().with_delivery(target)).await.unwrap();
    let first = presence_claim("test");
    let stale = store.claim_delivery(&first).await.unwrap().unwrap();
    let run = store
        .claim("runner", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store.record(&run.lease, &Event::RunStarted).await.unwrap();
    let replacement = DeliveryClaim::new(
        "test",
        "replacement",
        std::time::Duration::from_secs(30),
        32,
    )
    .unwrap()
    .with_presence(std::time::Duration::from_hours(1))
    .unwrap();

    let started = store.claim_delivery(&replacement).await.unwrap().unwrap();
    assert!(matches!(started.events[0].event, Event::RunStarted));
    let error = store
        .checkpoint_delivery(&DeliveryCheckpoint::presented(stale.lease))
        .await
        .expect_err("the empty presence lease is stale");
    assert_eq!(error.code(), "lease_lost");
}

#[tokio::test]
async fn a_terminal_reply_represents_a_live_same_lane_sibling() {
    let store = MemoryStore::default();
    let address = Address::new("test", "C123", "thread-42").unwrap();
    let first = Prompt::new(address.clone(), "event-a", "system", "A")
        .unwrap()
        .with_delivery(DeliveryTarget::new("test", json!({"recipient": "one"})).unwrap());
    let second = Prompt::new(address, "event-b", "system", "B")
        .unwrap()
        .with_delivery(DeliveryTarget::new("test", json!({"recipient": "two"})).unwrap());
    store.accept(&first).await.unwrap();
    let accepted = store.accept(&second).await.unwrap();
    let claim = DeliveryClaim::new("test", "surface", std::time::Duration::from_secs(30), 32)
        .unwrap()
        .with_presence(std::time::Duration::from_hours(1))
        .unwrap();
    present_next(&store, &claim).await;
    assert!(
        store.claim_delivery(&claim).await.unwrap().is_none(),
        "one lane and adapter has one live presence representative"
    );
    finish_next(&store).await;
    checkpoint_next(&store, &claim).await;

    let sibling = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(sibling.lease.operation(), accepted.operation.id);
    assert!(sibling.events.is_empty(), "the sibling is still accepted");
}

#[tokio::test]
async fn same_lane_terminal_deliveries_wait_for_their_predecessor() {
    let store = MemoryStore::default();
    let address = Address::new("test", "C123", "ordered-thread").unwrap();
    let target = DeliveryTarget::new("test", json!({})).unwrap();
    let first = Prompt::new(address.clone(), "ordered-a", "system", "A")
        .unwrap()
        .with_delivery(target.clone());
    let second = Prompt::new(address, "ordered-b", "system", "B")
        .unwrap()
        .with_delivery(target);
    let first_id = store.accept(&first).await.unwrap().operation.id;
    finish_next(&store).await;
    let second_id = store.accept(&second).await.unwrap().operation.id;
    finish_next(&store).await;
    let claim = presence_claim("test");

    let predecessor = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(predecessor.lease.operation(), first_id);
    assert!(store.claim_delivery(&claim).await.unwrap().is_none());
    let through = predecessor.events.last().unwrap().sequence;
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(predecessor.lease, through, None))
        .await
        .unwrap();
    let successor = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(successor.lease.operation(), second_id);
}

#[tokio::test]
async fn a_terminal_reply_does_not_touch_another_adapter() {
    let store = MemoryStore::default();
    let address = Address::new("surface", "conversation", "lane").unwrap();
    let first = Prompt::new(address.clone(), "event-a", "system", "A")
        .unwrap()
        .with_delivery(DeliveryTarget::new("test", json!({})).unwrap());
    let second = Prompt::new(address, "event-b", "system", "B")
        .unwrap()
        .with_delivery(DeliveryTarget::new("email", json!({})).unwrap());
    store.accept(&first).await.unwrap();
    store.accept(&second).await.unwrap();
    let claim = presence_claim("test");
    let email = presence_claim("email");
    present_next(&store, &claim).await;
    present_next(&store, &email).await;
    finish_next(&store).await;
    checkpoint_next(&store, &claim).await;

    assert!(store.claim_delivery(&email).await.unwrap().is_none());
}

#[tokio::test]
async fn terminal_reconciliation_fences_a_sibling_without_losing_its_stream() {
    let store = MemoryStore::default();
    let address = Address::new("test", "C123", "thread-42").unwrap();
    let first = Prompt::new(address.clone(), "event-a", "system", "A")
        .unwrap()
        .with_delivery(DeliveryTarget::new("test", json!({})).unwrap());
    let second = Prompt::new(address, "event-b", "system", "B")
        .unwrap()
        .with_delivery(DeliveryTarget::new("test", json!({})).unwrap());
    store.accept(&first).await.unwrap();
    let sibling = store.accept(&second).await.unwrap().operation.id;
    let claim = presence_claim("test");
    present_next(&store, &claim).await;
    assert!(store.claim_delivery(&claim).await.unwrap().is_none());
    finish_next(&store).await;
    let terminal = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert!(store.claim_delivery(&claim).await.unwrap().is_none());
    let stale = old_worker_lease(&store, sibling);
    let through = terminal.events.last().unwrap().sequence;
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(terminal.lease, through, None))
        .await
        .unwrap();

    let error = store
        .checkpoint_delivery(&DeliveryCheckpoint::presented(stale))
        .await
        .expect_err("the sibling presentation was fenced");
    assert_eq!(error.code(), "lease_lost");
    let replacement = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(replacement.external_id.as_deref(), Some("stream-b"));
}

fn old_worker_lease(store: &MemoryStore, operation: crate::OperationId) -> crate::DeliveryLease {
    let mut state = store.state.lock().unwrap();
    let row = state.deliveries.rows.get_mut(&operation).unwrap();
    row.token += 1;
    row.external_id = Some("stream-b".to_owned());
    row.presented = Some(std::time::Instant::now());
    row.lease = Some((
        "old-worker".to_owned(),
        row.token,
        std::time::Instant::now() + std::time::Duration::from_secs(30),
    ));
    crate::DeliveryLease {
        operation_id: operation,
        worker: "old-worker".to_owned(),
        token: row.token,
    }
}

fn presence_claim(adapter: &str) -> DeliveryClaim {
    DeliveryClaim::new(adapter, "surface", std::time::Duration::from_secs(30), 32)
        .unwrap()
        .with_presence(std::time::Duration::from_hours(1))
        .unwrap()
}

async fn present_next(store: &MemoryStore, claim: &DeliveryClaim) {
    let batch = store.claim_delivery(claim).await.unwrap().unwrap();
    assert!(batch.events.is_empty(), "the operation is still accepted");
    store
        .checkpoint_delivery(&DeliveryCheckpoint::presented(batch.lease))
        .await
        .unwrap();
}

async fn finish_next(store: &MemoryStore) {
    let run = store
        .claim("runner", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store.record(&run.lease, &Event::RunStarted).await.unwrap();
    store
        .record(
            &run.lease,
            &Event::RunFinished {
                outcome: Outcome::Completed {
                    text: "done".to_owned(),
                },
            },
        )
        .await
        .unwrap();
}

async fn checkpoint_next(store: &MemoryStore, claim: &DeliveryClaim) {
    let batch = store.claim_delivery(claim).await.unwrap().unwrap();
    let through = batch.events.last().unwrap().sequence;
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(batch.lease, through, None))
        .await
        .unwrap();
}

/// Postgres rejects a claim whose TTL cannot be represented as milliseconds
/// (`postgres_delivery_claim.rs` maps the overflow to an error before any
/// claim happens); the memory store must reject the same claim rather than
/// panic while computing `Instant::now() + ttl`.
#[tokio::test]
async fn a_claim_ttl_that_cannot_be_represented_is_rejected_not_panicked() {
    let store = MemoryStore::default();
    let prompt = prompt()
        .with_delivery(DeliveryTarget::new("test", json!({"conversation": "C123"})).unwrap());
    store.accept(&prompt).await.expect("accepted");
    let claim = DeliveryClaim::new("test", "surface", std::time::Duration::MAX, 32)
        .unwrap()
        .with_presence(std::time::Duration::from_millis(1))
        .unwrap();
    let result = store.claim_delivery(&claim).await;
    let error = result.expect_err("an unrepresentable claim TTL is an error");
    assert_eq!(error.code(), "invalid_input");
    let recover = DeliveryClaim::new("test", "surface", std::time::Duration::from_secs(30), 32)
        .unwrap()
        .with_presence(std::time::Duration::from_hours(1))
        .unwrap();
    assert!(
        store
            .claim_delivery(&recover)
            .await
            .expect("a representable TTL still claims")
            .is_some(),
        "the rejected claim must not leave the row locked or changed"
    );
}

/// The Postgres CHECKPOINT writes
/// `external_id = CASE WHEN $2 IS NULL THEN COALESCE($3, external_id) ELSE $3 END`,
/// so a presence checkpoint (`through` NULL) that learned an external id
/// persists it. The memory store must agree, or memory-store tests silently
/// certify semantics Postgres does not have.
#[tokio::test]
async fn a_presence_checkpoint_persists_a_learned_external_id() {
    let store = MemoryStore::default();
    let prompt = prompt()
        .with_delivery(DeliveryTarget::new("test", json!({"conversation": "C123"})).unwrap());
    store.accept(&prompt).await.expect("accepted");
    let claim = DeliveryClaim::new("test", "surface", std::time::Duration::from_secs(30), 32)
        .unwrap()
        .with_presence(std::time::Duration::from_millis(1))
        .unwrap();
    let batch = store.claim_delivery(&claim).await.unwrap().unwrap();
    store
        .checkpoint_delivery(&DeliveryCheckpoint {
            lease: batch.lease,
            through: None,
            external_id: Some("test-stream-2".to_owned()),
        })
        .await
        .expect("presence checkpoint");
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    let reclaim = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(reclaim.external_id, Some("test-stream-2".to_owned()));
}

#[tokio::test]
async fn steering_immediately_represents_a_caught_up_running_operation() {
    let store = MemoryStore::default();
    let target = DeliveryTarget::new("test", json!({"conversation": "C123"})).unwrap();
    let initial = prompt().with_delivery(target.clone());
    store.accept(&initial).await.expect("accepted");
    let claimed = store
        .claim("runner", std::time::Duration::from_secs(30))
        .await
        .expect("claim")
        .expect("pending");
    store
        .record(&claimed.lease, &Event::RunStarted)
        .await
        .expect("started");
    let delivery = DeliveryClaim::new("test", "surface-1", std::time::Duration::from_secs(30), 32)
        .unwrap()
        .with_presence(std::time::Duration::from_hours(1))
        .unwrap();
    let started = store.claim_delivery(&delivery).await.unwrap().unwrap();
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(
            started.lease,
            started.events[0].sequence,
            None,
        ))
        .await
        .expect("presented");
    let steering = Prompt::new(
        initial.address().clone(),
        "event-8",
        "be useful",
        "one more thing",
    )
    .unwrap()
    .with_delivery(target);

    store
        .admit_with_tools(&steering, &[])
        .await
        .expect("steering admitted");

    let live = store
        .claim_delivery(&delivery)
        .await
        .expect("claim")
        .expect("steering must make presentation due now");
    assert!(live.events.is_empty(), "steering is not consumed yet");
    store
        .checkpoint_delivery(&DeliveryCheckpoint::presented(live.lease))
        .await
        .expect("represented");
    store
        .admit_with_tools(&steering, &[])
        .await
        .expect("duplicate steering");
    assert!(
        store
            .claim_delivery(&delivery)
            .await
            .expect("claim")
            .is_none(),
        "an idempotent admission must not invalidate presentation again"
    );
}

#[tokio::test]
async fn steering_fences_an_in_flight_presentation() {
    let store = MemoryStore::default();
    let target = DeliveryTarget::new("test", json!({"conversation": "C123"})).unwrap();
    let initial = prompt().with_delivery(target.clone());
    store.accept(&initial).await.expect("accepted");
    let claimed = store
        .claim("runner", std::time::Duration::from_secs(30))
        .await
        .expect("claim")
        .expect("pending");
    store
        .record(&claimed.lease, &Event::RunStarted)
        .await
        .expect("started");
    let first =
        DeliveryClaim::new("test", "surface-1", std::time::Duration::from_secs(30), 32).unwrap();
    let started = store.claim_delivery(&first).await.unwrap().unwrap();
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(
            started.lease,
            started.events[0].sequence,
            None,
        ))
        .await
        .expect("presented");
    store
        .record(
            &claimed.lease,
            &Event::ToolStarted {
                id: "call-1".to_owned(),
                name: "search".to_owned(),
                args: json!({}),
            },
        )
        .await
        .expect("progress");
    let stale = store.claim_delivery(&first).await.unwrap().unwrap();
    let steering = Prompt::new(
        initial.address().clone(),
        "event-8",
        "be useful",
        "one more thing",
    )
    .unwrap()
    .with_delivery(target);

    store
        .admit_with_tools(&steering, &[])
        .await
        .expect("steering admitted");

    let second =
        DeliveryClaim::new("test", "surface-2", std::time::Duration::from_secs(30), 32).unwrap();
    assert!(
        store
            .claim_delivery(&second)
            .await
            .expect("reclaim")
            .is_some(),
        "steering must reclaim presentation without waiting for the stale lease"
    );
    let through = stale.events.last().unwrap().sequence;
    assert_eq!(
        store
            .checkpoint_delivery(&DeliveryCheckpoint::new(stale.lease, through, None))
            .await
            .expect_err("stale presentation")
            .code(),
        "lease_lost"
    );
}

#[tokio::test]
async fn a_failed_attempts_stream_identity_survives_into_the_reclaim() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let prompt = prompt()
        .with_delivery(DeliveryTarget::new("test", json!({"conversation": "C123"})).unwrap());
    runtime.accept(&prompt).await.expect("accepted");
    runtime
        .run_next(
            &StreamingModel(AtomicBool::new(false)),
            &Registry::default(),
        )
        .await
        .expect("run succeeds");

    let claim = DeliveryClaim::new(
        "test",
        "surface-1",
        std::time::Duration::from_millis(20),
        32,
    )
    .unwrap();
    let batch = store
        .claim_delivery(&claim)
        .await
        .expect("claim succeeds")
        .expect("events are pending");
    assert_eq!(batch.external_id, None);
    store
        .save_delivery_stream(&batch.lease, Some("stream-1"))
        .await
        .expect("stream identity saved under a live lease");

    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    let retry = store
        .claim_delivery(&claim)
        .await
        .expect("claim succeeds")
        .expect("nothing was checkpointed");
    assert_eq!(retry.external_id.as_deref(), Some("stream-1"));
    assert_eq!(
        retry.events.first().expect("events").sequence,
        batch.events.first().expect("events").sequence,
        "saving the stream must not advance the cursor"
    );
    assert_eq!(
        store
            .save_delivery_stream(&batch.lease, Some("stream-2"))
            .await
            .expect_err("stale lease")
            .code(),
        "lease_lost"
    );

    store
        .save_delivery_stream(&retry.lease, None)
        .await
        .expect("a dead stream identity clears");
    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    let reopened = store
        .claim_delivery(&claim)
        .await
        .expect("claim succeeds")
        .expect("still pending");
    assert_eq!(
        reopened.external_id, None,
        "the next attempt must open a fresh stream"
    );
}

#[tokio::test]
async fn a_second_replica_cannot_write_the_same_session() {
    let store = MemoryStore::default();
    let first = Runtime::new(store.clone());
    let second = Runtime::new(store);
    let address_a = Address::new("test", "C123", "a").expect("valid");
    let address_b = Address::new("test", "C123", "b").expect("valid");
    first
        .accept(&Prompt::new(address_a, "e1", "s", "one").expect("valid"))
        .await
        .expect("accepted");
    first
        .accept(&Prompt::new(address_b, "e2", "s", "two").expect("valid"))
        .await
        .expect("accepted");
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let model = Blocking {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    };
    let running = tokio::spawn(async move { first.run_next(&model, &Registry::default()).await });
    entered.notified().await;

    let claimed = second
        .run_next(&answer("must not run"), &Registry::default())
        .await
        .expect("claim check");

    assert!(claimed.is_none());
    release.notify_one();
    running.await.expect("task joined").expect("run succeeded");
}

#[tokio::test]
async fn a_crashed_run_resumes_after_its_lease_expires() {
    let store = MemoryStore::default();
    let ttl = std::time::Duration::from_millis(5);
    let first = Runtime::with_config(
        store.clone(),
        Config::new("worker-a", ttl).expect("valid config"),
    );
    let accepted = first.accept(&prompt()).await.expect("accepted");
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let model = Blocking {
        entered: Arc::clone(&entered),
        release,
    };
    let running = tokio::spawn(async move { first.run_next(&model, &Registry::default()).await });
    entered.notified().await;
    running.abort();
    drop(running.await);
    tokio::time::sleep(ttl.saturating_mul(2)).await;

    let reopened = Runtime::new(store);
    let finished = reopened
        .run_next(&answer("recovered"), &Registry::default())
        .await
        .expect("recovery succeeds")
        .expect("expired work is reclaimed");

    assert_eq!(finished.id, accepted.operation.id);
    assert_eq!(finished.state, OperationState::Completed);
}

#[tokio::test]
async fn a_live_run_renews_its_lease_during_slow_model_io() {
    let store = MemoryStore::default();
    let ttl = std::time::Duration::from_millis(9);
    let first = Runtime::with_config(
        store.clone(),
        Config::new("worker-a", ttl).expect("valid config"),
    );
    let second = Runtime::with_config(store, Config::new("worker-b", ttl).expect("valid config"));
    first.accept(&prompt()).await.expect("accepted");
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let model = Blocking {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    };
    let running = tokio::spawn(async move { first.run_next(&model, &Registry::default()).await });
    entered.notified().await;
    tokio::time::sleep(ttl.saturating_mul(2)).await;

    let stolen = second
        .run_next(&answer("stolen"), &Registry::default())
        .await
        .expect("claim check");

    assert!(stolen.is_none());
    release.notify_one();
    running.await.expect("task joined").expect("run succeeded");
}

#[tokio::test]
async fn a_tool_retry_reuses_its_idempotency_key_after_a_crash() {
    let store = MemoryStore::default();
    let ttl = std::time::Duration::from_millis(5);
    let first = Runtime::with_config(
        store.clone(),
        Config::new("worker-a", ttl).expect("valid config"),
    );
    first.accept(&prompt()).await.expect("accepted");
    let tool = RetryTool::new();
    let registry = Registry::new(vec![tool.clone()]);
    let first_registry = registry.clone();
    let running = tokio::spawn(async move { first.run_next(&ToolModel, &first_registry).await });
    tool.entered.notified().await;
    running.abort();
    drop(running.await);
    tokio::time::sleep(ttl.saturating_mul(2)).await;

    let reopened = Runtime::new(store);
    let finished = reopened
        .run_next(&answer("recovered"), &registry)
        .await
        .expect("recovery succeeds")
        .expect("work reclaimed");

    assert_eq!(finished.state, OperationState::Completed);
    let keys = tool.keys.lock().expect("keys poisoned");
    assert_eq!(keys.len(), 2);
    assert!(keys[0].is_some());
    assert_eq!(keys[0], keys[1]);
}

#[tokio::test]
async fn a_pre_upgrade_tool_retry_keeps_its_key_after_compaction_and_steering() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let tool = RetryTool::new();
    let registry = Registry::new(vec![tool.clone()]);
    let accepted = runtime
        .accept_with_tools(&prompt(), &registry)
        .await
        .unwrap();
    let ttl = std::time::Duration::from_millis(5);
    let claim = store.claim("old-writer", ttl).await.unwrap().unwrap();
    let response = ToolModel.generate(&claim.thread, &[]).await;
    let call = &response.tool_calls[0];
    let started = Event::ToolStarted {
        id: call.id.clone(),
        name: call.name.clone(),
        args: call.args.clone(),
    };
    // This is the old binary's durable boundary and external tool receipt.
    let key = format!(
        "operation/{}/step/2/call/{}",
        accepted.operation.id.get(),
        call.id
    );
    tool.keys.lock().unwrap().push(Some(key.clone()));
    for event in [
        Event::RunStarted,
        Event::Compacted {
            summary: "prior work".to_owned(),
            steps: 1,
        },
        Event::Appended {
            message: Message::User("steer".to_owned()),
        },
        Event::Appended {
            message: Message::Assistant(response),
        },
        started,
    ] {
        store.record(&claim.lease, &event).await.unwrap();
    }
    drop(claim);
    tokio::time::sleep(ttl.saturating_mul(2)).await;
    let recovered = runtime
        .run_next(&answer("recovered"), &registry)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.state, OperationState::Completed);
    assert_eq!(
        *tool.keys.lock().unwrap(),
        vec![Some(key.clone()), Some(key)]
    );
}

#[tokio::test]
async fn a_later_prompt_replays_the_lane_history_in_order() {
    let runtime = Runtime::new(MemoryStore::default());
    let first = prompt();
    runtime.accept(&first).await.expect("first accepted");
    runtime
        .run_next(&answer("first answer"), &Registry::default())
        .await
        .expect("first run")
        .expect("first work");
    let second =
        Prompt::new(first.address, "event-8", "be useful", "follow up").expect("valid prompt");
    runtime.accept(&second).await.expect("second accepted");
    let model = Capturing::new();

    runtime
        .run_next(&model, &Registry::default())
        .await
        .expect("second run")
        .expect("second work");

    let seen = model.seen.lock().expect("capture poisoned");
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0], Message::User("héllo ☕".to_owned()));
    assert!(matches!(&seen[1], Message::Assistant(response) if response.text == "first answer"));
    assert_eq!(seen[2], Message::User("follow up".to_owned()));
}

#[tokio::test]
async fn compaction_replaces_only_the_selected_history_prefix() {
    let runtime = Runtime::new(MemoryStore::default());
    let first_prompt = prompt();
    let first = runtime.accept(&first_prompt).await.expect("accepted");
    runtime
        .run_next(&answer("old answer"), &Registry::default())
        .await
        .expect("first run")
        .expect("first work");
    runtime
        .compact(&Compaction::new(
            first.operation.lane_id,
            first.operation.id,
            "The user greeted the agent.",
        ))
        .await
        .expect("compacted");
    let next =
        Prompt::new(first_prompt.address, "event-8", "be useful", "continue").expect("valid");
    runtime.accept(&next).await.expect("accepted");
    let model = Capturing::new();

    runtime
        .run_next(&model, &Registry::default())
        .await
        .expect("run")
        .expect("work");

    let seen = model.seen.lock().expect("capture poisoned");
    assert_eq!(
        seen[0],
        Message::Summary("The user greeted the agent.".to_owned())
    );
    assert_eq!(seen[1], Message::User("continue".to_owned()));
    assert_eq!(seen.len(), 2);
}

/// Postgres couples appends and lease renewal: `record` locks the session row
/// FOR UPDATE for its whole transaction, and `heartbeat` updates that same
/// row, so a heartbeat issued mid-append blocks until the append commits.
#[derive(Clone)]
struct RowLocked {
    inner: MemoryStore,
    row: Arc<tokio::sync::Mutex<()>>,
}

/// How long `record` keeps the row locked across awaits, standing in for the
/// statement round trips inside the real transaction. Longer than the drive
/// loop's heartbeat interval (lease TTL / 3) so a heartbeat fires mid-append.
const APPEND_TRANSACTION: std::time::Duration = std::time::Duration::from_millis(150);

impl Store for RowLocked {
    async fn accept(&self, prompt: &Prompt) -> Result<crate::Accepted, crate::Error> {
        self.inner.accept(prompt).await
    }

    async fn admit_with_tools(
        &self,
        prompt: &Prompt,
        definitions: &[Value],
    ) -> Result<crate::Admitted, crate::Error> {
        self.inner.admit_with_tools(prompt, definitions).await
    }

    async fn operation(
        &self,
        id: crate::OperationId,
    ) -> Result<Option<crate::Operation>, crate::Error> {
        self.inner.operation(id).await
    }

    async fn claim(
        &self,
        worker: &str,
        ttl: std::time::Duration,
    ) -> Result<Option<crate::Claimed>, crate::Error> {
        self.inner.claim(worker, ttl).await
    }

    async fn claim_operation(
        &self,
        id: crate::OperationId,
        worker: &str,
        ttl: std::time::Duration,
    ) -> Result<Option<crate::Claimed>, crate::Error> {
        self.inner.claim_operation(id, worker, ttl).await
    }

    async fn record(&self, lease: &crate::Lease, event: &Event) -> Result<(), crate::Error> {
        let _row = self.row.lock().await;
        tokio::time::sleep(APPEND_TRANSACTION).await;
        self.inner.record(lease, event).await
    }

    async fn settle(
        &self,
        lease: &crate::Lease,
        outcome: &Outcome,
    ) -> Result<crate::Settled, crate::Error> {
        self.inner.settle(lease, outcome).await
    }

    async fn heartbeat(
        &self,
        lease: &crate::Lease,
        ttl: std::time::Duration,
    ) -> Result<(), crate::Error> {
        let _row = self.row.lock().await;
        self.inner.heartbeat(lease, ttl).await
    }

    async fn compact(&self, compaction: &Compaction) -> Result<(), crate::Error> {
        self.inner.compact(compaction).await
    }
}

/// Regression: the drive loop awaited the heartbeat inline in its select
/// branch body, during which the run future was not polled. A heartbeat
/// blocked behind the run's open record transaction then deadlocked the
/// operation forever (observed live 2026-08-17, operation 39).
#[tokio::test(start_paused = true)]
async fn a_heartbeat_blocked_by_an_open_record_transaction_cannot_stall_the_run() {
    let store = RowLocked {
        inner: MemoryStore::default(),
        row: Arc::default(),
    };
    let config =
        Config::new("worker", std::time::Duration::from_millis(300)).expect("valid config");
    let runtime = Runtime::with_config(store, config);
    runtime.accept(&prompt()).await.expect("accepted");

    let model = answer("done");
    let tools = Registry::default();
    let run = runtime.run_next(&model, &tools);
    let operation = tokio::time::timeout(std::time::Duration::from_mins(1), run)
        .await
        .expect("deadlock: the run stopped being polled while a heartbeat waited on its row lock")
        .expect("run succeeds")
        .expect("operation ran");

    assert_eq!(operation.state, OperationState::Completed);
}

// A tool finds its own thread through the operation's durable delivery
// record, never through anything the model supplied.
#[tokio::test]
async fn an_operation_reports_the_delivery_target_it_was_accepted_with() {
    let store = MemoryStore::default();
    let address = Address::new("test", "conversation", "172.000").unwrap();
    let target =
        DeliveryTarget::new("test", json!({"conversation": "C1", "post_id": "172.000"})).unwrap();
    let routed = Prompt::new(address.clone(), "routed", "system", "hello")
        .unwrap()
        .with_delivery(target.clone());
    let bare = Prompt::new(address, "bare", "system", "hello").unwrap();
    let routed = store.accept(&routed).await.unwrap().operation.id;
    let bare = store.accept(&bare).await.unwrap().operation.id;

    assert_eq!(store.delivery_target(routed).await.unwrap(), Some(target));
    assert_eq!(store.delivery_target(bare).await.unwrap(), None);
}

/// Asks for the `fetch` tool once, then answers.
struct FetchThenAnswer(AtomicBool);

impl Model for FetchThenAnswer {
    async fn generate(&self, _thread: &Thread, _tools: &[Value]) -> ModelResponse {
        if self.0.swap(true, Ordering::SeqCst) {
            return answer("seen").0.into_inner().unwrap().unwrap();
        }
        ModelResponse {
            model: None,
            reasoning: Vec::new(),
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "call-1".to_owned(),
                name: "fetch".to_owned(),
                args: json!({}),
            }],
            stop: StopReason::ToolUse,
        }
    }
}

struct FetchTool;

impl Tool for FetchTool {
    fn name(&self) -> &'static str {
        "fetch"
    }

    fn definition(&self) -> Value {
        json!({"name": "fetch", "description": "fetches", "input_schema": {"type": "object"}})
    }

    fn call<'a>(
        &'a self,
        _request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<agent::ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async {
            Ok(agent::ToolOutput {
                text: "fetched".to_owned(),
                images: vec![fetched_image()],
                files: vec![fetched_file()],
            })
        })
    }
}

// A fetched file is journaled once, in the message that rebuilds the thread;
// the progress event that rebuilds nothing carries only the text.
#[tokio::test]
async fn a_fetched_file_is_journaled_only_in_the_appended_result() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let target = DeliveryTarget::new("files", json!({"conversation": "C1"})).unwrap();
    runtime
        .accept(&prompt().with_delivery(target))
        .await
        .unwrap();
    let tools = Registry::new(vec![Arc::new(FetchTool)]);
    runtime
        .run_next(&FetchThenAnswer(AtomicBool::new(false)), &tools)
        .await
        .unwrap()
        .unwrap();

    let claim =
        DeliveryClaim::new("files", "surface", std::time::Duration::from_secs(30), 32).unwrap();
    let batch = store.claim_delivery(&claim).await.unwrap().unwrap();
    let images = |event: &Event| match event {
        Event::ToolFinished { result } => Some(("finished", result.images.len())),
        Event::Appended {
            message: Message::ToolResults(results),
        } => Some(("appended", results[0].images.len())),
        _ => None,
    };
    let seen: Vec<_> = batch
        .events
        .iter()
        .filter_map(|item| images(&item.event))
        .collect();
    assert_eq!(seen, [("finished", 0), ("appended", 1)]);
}

// A row this build cannot interpret, such as one stored while video was a
// supported tool output, is rejected rather than guessed at.
#[test]
fn a_stored_video_row_is_rejected() {
    let row = ("video/mp4".to_owned(), vec![4, 5, 6], None, None, None);
    let error = crate::stored_media_row::parse(row).expect_err("video rows are unreadable");
    assert_eq!(error.code(), "corrupt_record");
}

fn fetched_image() -> Image {
    Image::new(ImageMediaType::Png, vec![1, 2, 3], 2, 3).unwrap()
}

fn fetched_file() -> agent::File {
    agent::File::new("compte rendu ☕.md", "text/markdown", b"# Notes".to_vec()).unwrap()
}

// A reply names a tool's image or file by reference, and delivery reads it
// from the store rather than the thread: the turn's own reply and later turns
// of any lane under the same root resolve it, another conversation does not.
#[tokio::test]
async fn delivery_resolves_the_media_a_conversations_tools_returned() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    runtime.accept(&prompt()).await.unwrap();
    let tools = Registry::new(vec![Arc::new(FetchTool)]);
    let fetched = runtime
        .run_next(&FetchThenAnswer(AtomicBool::new(false)), &tools)
        .await
        .unwrap()
        .unwrap()
        .id;
    let later = Prompt::new(prompt().address, "event-8", "be useful", "again").unwrap();
    let later = runtime.accept(&later).await.unwrap().operation.id;
    let elsewhere = Address::new("test", "C999", "thread-1").unwrap();
    let elsewhere = Prompt::new(elsewhere, "event-9", "be useful", "hi").unwrap();
    let elsewhere = runtime.accept(&elsewhere).await.unwrap().operation.id;
    let image = media::reference(&fetched_image());
    let file = media::file_reference(&fetched_file());

    let own = store.media(fetched, &image).await.unwrap();
    let written = store.media(later, &file).await.unwrap();

    assert_eq!(own, Some(crate::Media::Image(fetched_image())));
    assert_eq!(written, Some(crate::Media::File(fetched_file())));
    assert_eq!(store.media(elsewhere, &file).await.unwrap(), None);
    assert_eq!(store.media(elsewhere, &image).await.unwrap(), None);
    assert_eq!(
        store
            .media(fetched, "image:0000000000000000")
            .await
            .unwrap(),
        None
    );
    let unknown = crate::OperationId::new(9_999).unwrap();
    assert_eq!(store.media(unknown, &image).await.unwrap(), None);
}

// A subagent's tools run in its own session, yet the parent's reply is what
// shows their output: media resolves under the root conversation.
#[tokio::test]
async fn a_subagents_media_resolves_for_its_parents_reply() {
    let store = MemoryStore::default();
    let runtime = Runtime::new(store.clone());
    let parent = runtime.accept(&prompt()).await.unwrap().operation.id;
    store
        .claim("parent", std::time::Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let config = SubagentConfig::new("recorder", "record system", ExecutionProfile::default())
        .unwrap()
        .with_tools(vec![FetchTool.definition()]);
    let request = SubagentRequest::new(parent, "spawn-key", "record it", config).unwrap();
    runtime.spawn_subagent(&request).await.unwrap();
    let tools = Registry::new(vec![Arc::new(FetchTool)]);
    runtime
        .run_next(&FetchThenAnswer(AtomicBool::new(false)), &tools)
        .await
        .unwrap()
        .unwrap();

    let file = store
        .media(parent, &media::file_reference(&fetched_file()))
        .await
        .unwrap();

    assert_eq!(file, Some(crate::Media::File(fetched_file())));
}
