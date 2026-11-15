//! Output correction uses the same optional response limit as execution.
#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test models run when polled."
)]

use brainstone_agent::{
    ExecutionProfile, Message, Model, ModelResponse, StopReason, Thread, output_feedback,
};

struct Correcting;

impl Model for Correcting {
    async fn generate(&self, _: &Thread, _: &[serde_json::Value]) -> ModelResponse {
        unreachable!("feedback does not generate a response")
    }

    async fn output_feedback(&self, _: &Thread) -> Option<String> {
        Some("repair output".into())
    }
}

#[tokio::test]
async fn unlimited_output_can_be_corrected_after_one_hundred_responses() {
    let thread = Thread::restore_compacted(
        "system",
        vec![answer()],
        ExecutionProfile::default(),
        brainstone_agent::MAX_STEPS,
    )
    .unwrap();
    assert_eq!(
        output_feedback(&Correcting, &thread).await,
        Some("repair output".into())
    );
}

#[tokio::test]
async fn a_finite_limit_still_prevents_an_extra_correction_response() {
    let profile = ExecutionProfile::new(None, None, brainstone_agent::MAX_STEPS, 0).unwrap();
    let thread = Thread::restore_compacted(
        "system",
        vec![answer()],
        profile,
        brainstone_agent::MAX_STEPS,
    )
    .unwrap();
    assert_eq!(output_feedback(&Correcting, &thread).await, None);
}

#[tokio::test]
async fn unlimited_execution_still_permits_only_one_output_correction() {
    let mut thread = Thread::new("system", "work");
    thread.push(Message::OutputCorrection("repair output".into()));
    thread.push(answer());
    assert_eq!(output_feedback(&Correcting, &thread).await, None);
}

fn answer() -> Message {
    Message::Assistant(ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: "invalid output".into(),
        tool_calls: Vec::new(),
        stop: StopReason::EndTurn,
    })
}
