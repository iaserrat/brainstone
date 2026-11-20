#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Behaviour of the driver against scripted providers and tools. The point of
//! this file is the table of runs, so it is exempt from the line budget.
#![allow(clippy::too_many_lines)]

use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use crate::event::{Event, Sink};
use crate::model::{ModelResponse, StopReason, ToolCall};
use crate::provider::Model;
use crate::registry::Registry;
use crate::run::run;
use crate::step::{CODE_TRUNCATED_CALL, CODE_UNKNOWN_TOOL, Outcome};
use crate::thread::{Message, Thread};
use crate::tool::{Tool, ToolError, ToolRequest};
use crate::tool_bound::{CODE_TOOL_RESULT_TRUNCATED, MAX_TOOL_RESULT_BYTES};

/// A provider that replays a fixed script, one response per call.
struct Scripted {
    responses: Mutex<Vec<ModelResponse>>,
    /// Tool lists the loop offered, so a test can assert what the model saw.
    offered: Mutex<Vec<Vec<Value>>>,
}

struct Streaming;

impl Model for Streaming {
    async fn generate(&self, _thread: &Thread, _tools: &[Value]) -> ModelResponse {
        answer("héllo ☕")
    }

    async fn generate_stream<S: Sink + Send>(
        &self,
        _thread: &Thread,
        _tools: &[Value],
        sink: &mut S,
    ) -> Result<ModelResponse, S::Error> {
        sink.emit(Event::ModelDelta {
            delta: "hé".to_owned(),
        })
        .await?;
        sink.emit(Event::ModelDelta {
            delta: "llo ☕".to_owned(),
        })
        .await?;
        Ok(answer("héllo ☕"))
    }
}

impl Scripted {
    fn new(responses: Vec<ModelResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().rev().collect()),
            offered: Mutex::new(Vec::new()),
        }
    }
}

impl Model for Scripted {
    async fn generate(&self, _thread: &Thread, tools: &[Value]) -> ModelResponse {
        self.offered.lock().expect("poisoned").push(tools.to_vec());
        self.responses
            .lock()
            .expect("poisoned")
            .pop()
            .expect("the script ran out of responses")
    }
}

const NAME: &str = "echo";

/// A tool that counts its calls and answers with whatever it was handed,
/// plus an image when told to fetch one.
struct Echo {
    calls: AtomicUsize,
    fail: Option<ToolError>,
    image: Option<crate::Image>,
}

impl Echo {
    fn ok() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            fail: None,
            image: None,
        })
    }

    fn failing(code: &'static str) -> Arc<Self> {
        Self::failing_with(code, "the upstream said no".to_owned())
    }

    fn failing_with(code: &'static str, message: String) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            fail: Some(ToolError::new(code, message)),
            image: None,
        })
    }

    fn fetching(image: crate::Image) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            fail: None,
            image: Some(image),
        })
    }

    fn count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Tool for Echo {
    fn name(&self) -> &str {
        NAME
    }

    fn definition(&self) -> Value {
        json!({ "name": "echo", "description": "echoes", "input_schema": { "type": "object" } })
    }

    fn call<'a>(
        &'a self,
        request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<crate::ToolOutput, ToolError>> + Send + 'a>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            match &self.fail {
                Some(e) => Err(e.clone()),
                None => Ok(crate::ToolOutput {
                    text: request.args.to_string(),
                    images: self.image.clone().into_iter().collect(),
                    ..crate::ToolOutput::default()
                }),
            }
        })
    }
}

/// Records everything, and can be told to fail on the nth write.
#[derive(Default)]
pub(crate) struct Recorder {
    pub(crate) events: Vec<Event>,
    fail_at: Option<usize>,
}

impl Recorder {
    fn failing_at(n: usize) -> Self {
        Self {
            events: Vec::new(),
            fail_at: Some(n),
        }
    }

    /// The thread a durable consumer rebuilds: what it already held when the
    /// run started, plus every message the run recorded appending.
    fn replay(&self, start: Thread) -> Thread {
        let mut thread = start;
        for event in &self.events {
            if let Event::Appended { message } = event {
                thread.push(message.clone());
            }
        }
        thread
    }
}

impl Sink for Recorder {
    type Error = &'static str;

    async fn emit(&mut self, event: Event) -> Result<(), Self::Error> {
        if self.fail_at == Some(self.events.len()) {
            return Err("the store went away");
        }
        self.events.push(event);
        Ok(())
    }
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

fn asks(stop: StopReason, ids: &[&str]) -> ModelResponse {
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: String::new(),
        tool_calls: ids
            .iter()
            .map(|id| ToolCall {
                id: (*id).to_owned(),
                name: "echo".to_owned(),
                args: json!({ "say": "héllo ☕" }),
            })
            .collect(),
        stop,
    }
}

fn results_of(thread: &Thread) -> Vec<&crate::tool::ToolResult> {
    thread
        .messages()
        .iter()
        .filter_map(|m| match m {
            Message::ToolResults(r) => Some(r),
            _ => None,
        })
        .flatten()
        .collect()
}

#[tokio::test]
async fn a_fetched_image_is_recorded_inside_the_tool_result() {
    let image = crate::Image::new(crate::ImageMediaType::Png, vec![1, 2, 3], 2, 3).unwrap();
    let registry = Registry::new(vec![Echo::fetching(image.clone())]);
    let model = Scripted::new(vec![asks(StopReason::ToolUse, &["c1"]), answer("seen")]);
    let start = Thread::new("s", "look at it");
    let mut thread = start.clone();
    let mut sink = Recorder::default();

    run(&mut thread, &model, &registry, &mut sink)
        .await
        .expect("the sink never fails");

    let rebuilt = sink.replay(start);
    let results = results_of(&rebuilt);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].images, vec![image]);
    assert_eq!(results[0].content, json!({ "say": "héllo ☕" }).to_string());
    let finished = sink.events.iter().find_map(|event| match event {
        Event::ToolFinished { result } => Some(result),
        _ => None,
    });
    assert_eq!(finished.expect("progress event").images, []);
}

#[tokio::test]
async fn a_plain_answer_ends_the_run() {
    let mut thread = Thread::new("s", "hello");
    let model = Scripted::new(vec![answer("hi ☺")]);
    let mut sink = Recorder::default();

    let outcome = run(&mut thread, &model, &Registry::default(), &mut sink)
        .await
        .expect("the sink never fails");

    assert_eq!(
        outcome,
        Outcome::Completed {
            text: "hi ☺".to_owned()
        }
    );
    assert_eq!(thread.steps(), 1);
    assert!(matches!(sink.events.first(), Some(Event::RunStarted)));
    assert!(matches!(
        sink.events.last(),
        Some(Event::RunFinished { .. })
    ));
}

#[tokio::test]
async fn model_deltas_are_ordered_before_the_settled_message() {
    let mut thread = Thread::new("s", "hello");
    let mut sink = Recorder::default();

    run(&mut thread, &Streaming, &Registry::default(), &mut sink)
        .await
        .expect("the sink never fails");

    assert!(matches!(
        sink.events.as_slice(),
        [
            Event::RunStarted,
            Event::ModelDelta { delta: first },
            Event::ModelDelta { delta: second },
            Event::Appended { .. },
            Event::RunFinished { .. }
        ] if first == "hé" && second == "llo ☕"
    ));
}

#[tokio::test]
async fn a_tool_call_runs_and_its_result_returns_to_the_model() {
    let echo = Echo::ok();
    let registry = Registry::new(vec![echo.clone()]);
    let model = Scripted::new(vec![asks(StopReason::ToolUse, &["c1"]), answer("done")]);
    let mut thread = Thread::new("s", "hello");
    let mut sink = Recorder::default();

    let outcome = run(&mut thread, &model, &registry, &mut sink)
        .await
        .expect("the sink never fails");

    assert_eq!(
        outcome,
        Outcome::Completed {
            text: "done".to_owned()
        }
    );
    assert_eq!(echo.count(), 1);
    let results = results_of(&thread);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "c1");
    assert!(!results[0].is_error);
    assert!(
        sink.events
            .iter()
            .any(|e| matches!(e, Event::ToolStarted { id, .. } if id == "c1"))
    );
}

#[tokio::test]
async fn the_model_is_offered_the_registry_definitions() {
    let registry = Registry::new(vec![Echo::ok()]);
    let model = Scripted::new(vec![answer("hi")]);
    let mut thread = Thread::new("s", "hello");

    run(&mut thread, &model, &registry, &mut Recorder::default())
        .await
        .expect("the sink never fails");

    let offered = model.offered.lock().expect("poisoned");
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0], registry.definitions());
    assert_eq!(offered[0][0]["name"], "echo");
}

#[tokio::test]
async fn every_call_of_a_batch_is_answered() {
    let echo = Echo::ok();
    let registry = Registry::new(vec![echo.clone()]);
    let model = Scripted::new(vec![
        asks(StopReason::ToolUse, &["c1", "c2", "c3"]),
        answer("done"),
    ]);
    let mut thread = Thread::new("s", "hello");

    run(&mut thread, &model, &registry, &mut Recorder::default())
        .await
        .expect("the sink never fails");

    assert_eq!(echo.count(), 3);
    let ids: Vec<&str> = results_of(&thread).iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["c1", "c2", "c3"], "results keep the requested order");
}

#[tokio::test]
async fn an_unknown_tool_is_an_error_result_not_a_dead_run() {
    let model = Scripted::new(vec![
        asks(StopReason::ToolUse, &["c1"]),
        answer("recovered"),
    ]);
    let mut thread = Thread::new("s", "hello");

    let outcome = run(
        &mut thread,
        &model,
        &Registry::default(),
        &mut Recorder::default(),
    )
    .await
    .expect("the sink never fails");

    assert_eq!(
        outcome,
        Outcome::Completed {
            text: "recovered".to_owned()
        }
    );
    let results = results_of(&thread);
    assert!(results[0].is_error);
    let doc: Value = serde_json::from_str(&results[0].content).expect("a structured document");
    assert_eq!(doc["error"], CODE_UNKNOWN_TOOL);
}

#[tokio::test]
async fn a_failing_tool_reports_its_own_stable_code() {
    let registry = Registry::new(vec![Echo::failing("upstream_refused")]);
    let model = Scripted::new(vec![asks(StopReason::ToolUse, &["c1"]), answer("ok")]);
    let mut thread = Thread::new("s", "hello");

    run(&mut thread, &model, &registry, &mut Recorder::default())
        .await
        .expect("the sink never fails");

    let results = results_of(&thread);
    assert!(results[0].is_error);
    let doc: Value = serde_json::from_str(&results[0].content).expect("a structured document");
    assert_eq!(doc["error"], "upstream_refused");
}

/// The case worth the whole `FailTruncated` branch: the batch is answered, but
/// nothing runs, so a half-parsed argument list cannot cause an effect.
#[tokio::test]
async fn a_truncated_batch_is_answered_without_running_anything() {
    let echo = Echo::ok();
    let registry = Registry::new(vec![echo.clone()]);
    let model = Scripted::new(vec![
        asks(StopReason::MaxTokens, &["c1", "c2"]),
        answer("reissued"),
    ]);
    let mut thread = Thread::new("s", "hello");
    let mut sink = Recorder::default();

    run(&mut thread, &model, &registry, &mut sink)
        .await
        .expect("the sink never fails");

    assert_eq!(echo.count(), 0, "a truncated call must never execute");
    let results = results_of(&thread);
    assert_eq!(results.len(), 2);
    for result in results {
        assert!(result.is_error);
        let doc: Value = serde_json::from_str(&result.content).expect("a structured document");
        assert_eq!(doc["error"], CODE_TRUNCATED_CALL);
    }
    assert!(
        !sink
            .events
            .iter()
            .any(|e| matches!(e, Event::ToolStarted { .. })),
        "nothing started, so nothing reports having started"
    );
}

/// The durability contract, held without a durable store existing yet.
#[tokio::test]
async fn the_event_stream_alone_rebuilds_the_thread() {
    let registry = Registry::new(vec![Echo::ok()]);
    let model = Scripted::new(vec![
        asks(StopReason::ToolUse, &["c1", "c2"]),
        asks(StopReason::MaxTokens, &["c3"]),
        answer("finally ✅"),
    ]);
    let mut thread = Thread::new("s", "hello");
    let mut sink = Recorder::default();

    run(&mut thread, &model, &registry, &mut sink)
        .await
        .expect("the sink never fails");

    let rebuilt = sink.replay(Thread::new("s", "hello"));
    assert_eq!(rebuilt, thread);
}

#[tokio::test]
async fn a_sink_that_cannot_write_stops_the_run() {
    let model = Scripted::new(vec![answer("hi")]);
    let mut thread = Thread::new("s", "hello");

    let failed = run(
        &mut thread,
        &model,
        &Registry::default(),
        &mut Recorder::failing_at(0),
    )
    .await;

    assert_eq!(failed, Err("the store went away"));
}

/// A message reaches the sink before it reaches the thread, so a store that
/// dies mid-run is ahead of memory rather than behind it. Replay can drop a
/// duplicate; it cannot invent a message it never saw.
#[tokio::test]
async fn a_message_is_recorded_before_it_is_appended() {
    let model = Scripted::new(vec![answer("hi")]);
    let mut thread = Thread::new("s", "hello");

    // Event 0 is RunStarted, so failing at 1 fails on the assistant message.
    let failed = run(
        &mut thread,
        &model,
        &Registry::default(),
        &mut Recorder::failing_at(1),
    )
    .await;

    assert!(failed.is_err());
    assert_eq!(thread.steps(), 0, "the unrecorded message never landed");
}

/// What one call with `args` leaves in the thread, run through `echo`.
async fn echoed(echo: Arc<Echo>, args: Value) -> String {
    let call = ToolCall {
        id: "c1".to_owned(),
        name: NAME.to_owned(),
        args,
    };
    crate::execute_tool(&call, &Registry::new(vec![echo]), None)
        .await
        .content
}

/// The marker line and the byte count it says was omitted.
fn marker(bounded: &str) -> (&str, usize) {
    let line = bounded
        .lines()
        .find(|line| line.starts_with(&format!("[{CODE_TOOL_RESULT_TRUNCATED}: ")))
        .expect("a marker line naming the stable code");
    let omitted = line
        .split(' ')
        .nth(1)
        .and_then(|value| value.parse().ok())
        .expect("the marker states how many bytes were omitted");
    (line, omitted)
}

/// Every later request replays a result, so one call must not be able to
/// outgrow the context window. A result at the cap is the tool's own bytes.
#[tokio::test]
async fn a_result_within_the_cap_reaches_the_thread_unchanged() {
    let args = json!({ "say": "é".repeat(MAX_TOOL_RESULT_BYTES / 2 - 5) });
    assert_eq!(args.to_string().len(), MAX_TOOL_RESULT_BYTES);

    assert_eq!(echoed(Echo::ok(), args.clone()).await, args.to_string());
}

/// Recorded shape of the incident: a fetched binary inlined as text, megabytes
/// of it. The head and tail survive around a marker naming the stable code and
/// exactly how much was cut, on character boundaries.
#[tokio::test]
async fn an_oversized_result_keeps_its_head_and_tail_around_a_marker() {
    let body = "é☕x".repeat(4 * 1024 * 1024 / "é☕x".len());
    let args = json!({ "say": format!("HEAD☕{body}☕TAIL") });
    let text = args.to_string();

    let bounded = echoed(Echo::ok(), args).await;

    assert!(
        bounded.len() < MAX_TOOL_RESULT_BYTES + 200,
        "{}",
        bounded.len()
    );
    assert!(
        bounded.starts_with(r#"{"say":"HEAD☕"#),
        "{}",
        &bounded[..20]
    );
    assert!(bounded.ends_with(r#"☕TAIL"}"#));
    let (marker, omitted) = marker(&bounded);
    let kept = bounded.len() - marker.len() - 2;
    assert_eq!(kept + omitted, text.len(), "{marker}");
    assert!(
        marker.contains(&format!("of {} bytes", text.len())),
        "{marker}"
    );
}

/// A failure's document must stay parseable, so the bound applies to its
/// message before the document is built, never to the document itself.
#[tokio::test]
async fn an_oversized_failure_stays_a_structured_document() {
    let message = "x".repeat(3 * MAX_TOOL_RESULT_BYTES);
    let echo = Echo::failing_with("upstream_refused", message);

    let content = echoed(echo, json!({})).await;

    let document: Value = serde_json::from_str(&content).expect("a structured document");
    assert_eq!(document["error"], "upstream_refused");
    let message = document["message"].as_str().expect("a message");
    assert!(message.len() < MAX_TOOL_RESULT_BYTES + 200);
    assert!(message.contains(CODE_TOOL_RESULT_TRUNCATED));
}
