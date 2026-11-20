//! Deterministic provider substitute; the example needs no credentials.
use brainstone::{
    Model,
    agent::{Message, ModelResponse, StopReason, Thread, ToolCall},
};
use serde_json::{Value, json};

pub(crate) struct LocalModel;

impl Model for LocalModel {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "Deterministic local example model."
    )]
    async fn generate(&self, thread: &Thread, _: &[Value]) -> ModelResponse {
        let mut response = ModelResponse {
            text: String::new(),
            reasoning: vec![],
            tool_calls: vec![],
            stop: StopReason::EndTurn,
            model: Some("local".into()),
        };
        match thread.last() {
            Message::User(_) => {
                response.stop = StopReason::ToolUse;
                response.tool_calls.push(ToolCall {
                    id: "refund-1".into(),
                    name: "Refund".into(),
                    args: json!({"order_id":"BS-1001","amount_cents":500}),
                });
            }
            Message::ToolResults(results) => response.text.clone_from(&results[0].content),
            _ => response.text = "No simulated refund result.".into(),
        }
        response
    }
}
