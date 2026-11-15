#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Capability cases exercise each unsupported request shape.
#![allow(clippy::too_many_lines)]
use brainstone_agent::{
    CapabilityModel, Message, Model, ModelCapabilities, ModelResponse, Sink, StopReason, Thread,
};
use serde_json::{Value, json};

struct Answer;
impl Model for Answer {
    async fn generate(&self, _: &Thread, _: &[Value]) -> ModelResponse {
        ModelResponse {
            model: None,
            text: "ok".to_owned(),
            reasoning: vec![],
            tool_calls: vec![],
            stop: StopReason::EndTurn,
        }
    }
}

#[test]
fn unsupported_inputs_are_named_without_dropping_content() {
    let image =
        brainstone_agent::Image::new(brainstone_agent::ImageMediaType::Png, vec![1], 1, 1).unwrap();
    let capabilities = ModelCapabilities {
        tools: false,
        images: false,
        tool_images: false,
        reasoning_effort: false,
        streaming: false,
    };
    let tools = capabilities
        .validate(&Thread::new("s", "go"), true)
        .unwrap_err();
    assert_eq!(tools.code(), "unsupported_tools");
    let cases = [
        (
            Message::UserImages(
                brainstone_agent::UserImages::new("", vec![image.clone()]).unwrap(),
            ),
            "unsupported_images",
        ),
        (
            Message::ToolResults(vec![result(vec![image])]),
            "unsupported_tool_images",
        ),
    ];
    for (message, code) in cases {
        let thread = Thread::restore("s", vec![message]).unwrap();
        assert_eq!(
            capabilities.validate(&thread, false).unwrap_err().code(),
            code
        );
        assert!(ModelCapabilities::default().validate(&thread, true).is_ok());
    }
    let thread = Thread::new_with_profile(
        "s",
        "go",
        brainstone_agent::ExecutionProfile::new(None, Some(brainstone_agent::Effort::High), 10, 0)
            .unwrap(),
    );
    assert_eq!(
        capabilities.validate(&thread, false).unwrap_err().code(),
        "unsupported_reasoning_effort"
    );
}

fn result(images: Vec<brainstone_agent::Image>) -> brainstone_agent::ToolResult {
    brainstone_agent::ToolResult {
        id: "call".to_owned(),
        name: "lookup".to_owned(),
        content: "found".to_owned(),
        is_error: false,
        images,
    }
}

struct Reject;
impl Sink for Reject {
    type Error = &'static str;
    async fn emit(&mut self, _: brainstone_agent::Event) -> Result<(), Self::Error> {
        Err("unexpected stream")
    }
}

#[tokio::test]
async fn settled_only_targets_and_declared_context_windows_work_through_the_same_seam() {
    let capabilities = ModelCapabilities {
        streaming: false,
        tools: false,
        ..ModelCapabilities::default()
    };
    let model = CapabilityModel::new(Answer, capabilities)
        .with_context_window(std::num::NonZeroUsize::new(4096).unwrap());
    assert_eq!(model.context_window(), Some(4096));
    let response = model
        .generate_stream(&Thread::new("s", "go"), &[], &mut Reject)
        .await
        .unwrap();
    assert_eq!(response.text, "ok");
    let response = model.generate(&Thread::new("s", "go"), &[json!({})]).await;
    assert!(matches!(response.stop,StopReason::Error {ref code,..} if code=="unsupported_tools"));
}
