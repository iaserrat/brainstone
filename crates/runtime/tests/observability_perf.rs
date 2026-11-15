#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Repeatable local hot-path check; run with --release --ignored --nocapture.
#![allow(clippy::too_many_lines)] // Repeated benchmark iterations and measurement setup.
use agent::{Model, ModelResponse, Registry, StopReason, Thread};
use brainstone_runtime::{Address, MemoryStore, Prompt, Runtime};
use std::time::Instant;
struct Answer;
impl Model for Answer {
    async fn generate(&self, _: &Thread, _: &[serde_json::Value]) -> ModelResponse {
        ModelResponse {
            reasoning: vec![],
            text: "ok".into(),
            tool_calls: vec![],
            stop: StopReason::EndTurn,
            model: None,
        }
    }
}
#[tokio::test]
#[ignore = "local performance measurement"]
async fn operation_latency() {
    let mut times = Vec::with_capacity(10_000);
    for _ in 0..10_100 {
        let runtime = Runtime::new(MemoryStore::default());
        let prompt = Prompt::new(
            Address::new("test", "channel", "thread").unwrap(),
            "event",
            "system",
            "prompt",
        )
        .unwrap();
        let start = Instant::now();
        runtime.accept(&prompt).await.unwrap();
        runtime
            .run_next(&Answer, &Registry::default())
            .await
            .unwrap()
            .unwrap();
        times.push(start.elapsed().as_nanos());
    }
    let mut times = times.split_off(100);
    times.sort_unstable();
    println!(
        "operation latency ns p50={} p95={}",
        times[5_000], times[9_500]
    );
}

struct FileAnswer;
impl Model for FileAnswer {
    async fn generate(&self, thread: &Thread, _: &[serde_json::Value]) -> ModelResponse {
        if let agent::Message::ToolResults(results) = thread.last() {
            assert!(!results[0].is_error);
        }
        let mut result = Answer.generate(thread, &[]).await;
        if matches!(thread.last(), agent::Message::User(_)) {
            result.tool_calls = vec![agent::ToolCall {
                id: "file".into(),
                name: "CreateFile".into(),
                args: serde_json::json!({"name":"bench.txt","content":"bench"}),
            }];
            result.stop = StopReason::ToolUse;
        }
        result
    }
}
#[tokio::test]
#[ignore = "local tool dispatch performance measurement"]
async fn tool_dispatch_latency() {
    let tools = Registry::new(vec![std::sync::Arc::new(media::CreateFile)]);
    let mut times = Vec::with_capacity(10_100);
    for _ in 0..10_100 {
        let runtime = Runtime::new(MemoryStore::default());
        let prompt = Prompt::new(
            Address::new("bench", "session", "lane").unwrap(),
            "one",
            "system",
            "go",
        )
        .unwrap();
        let start = Instant::now();
        runtime.accept(&prompt).await.unwrap();
        runtime
            .run_next(&FileAnswer, &tools)
            .await
            .unwrap()
            .unwrap();
        times.push(start.elapsed().as_nanos());
    }
    let mut times = times.split_off(100);
    times.sort_unstable();
    println!(
        "tool dispatch latency ns p50={} p95={}",
        times[5_000], times[9_500]
    );
}
