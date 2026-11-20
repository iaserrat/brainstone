//! Deterministic tool turns use the same driver as a real model.
use brainstone::{
    Model,
    agent::{Message, ModelResponse, StopReason, Thread, ToolCall},
};
use serde_json::{Value, json};

pub(crate) struct LocalModel;
impl Model for LocalModel {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "Deterministic offline model."
    )]
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        if let Message::ToolResults(results) = thread.last() {
            if results.iter().any(|result| result.is_error) {
                return response(StopReason::Error {
                    code: "demo_tool_failed".into(),
                    message: results[0].content.clone(),
                });
            }
            return match results[0].name.as_str() {
                "ReadFile" => call("EditFile", super::demo::edit_args()),
                "EditFile" => call("RunChecks", json!({})),
                _ => finished(),
            };
        }
        call("ReadFile", json!({"path":"src/lib.rs"}))
    }
}
fn response(stop: StopReason) -> ModelResponse {
    ModelResponse {
        model: Some("coding-demo".into()),
        text: String::new(),
        reasoning: vec![],
        tool_calls: vec![],
        stop,
    }
}
fn call(name: &str, args: Value) -> ModelResponse {
    let mut response = response(StopReason::ToolUse);
    response.tool_calls.push(ToolCall {
        id: name.into(),
        name: name.into(),
        args,
    });
    response
}
fn finished() -> ModelResponse {
    let mut response = response(StopReason::EndTurn);
    response.text = json!({"summary":"Fixed add and passed the Rust library tests.","files":["src/lib.rs"],"checks":"passed"}).to_string();
    response
}
