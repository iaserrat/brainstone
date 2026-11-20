use brainstone::Model;
use brainstone::agent::{Message, ModelResponse, Sink, Thread};
use brainstone::openai::ChatCompletionsModel;
use serde_json::Value;

pub(crate) struct EvalModel {
    pub(crate) inner: ChatCompletionsModel,
    pub(crate) crash_after_tool_result: bool,
}

impl EvalModel {
    fn boundary(&self, thread: &Thread) {
        if self.crash_after_tool_result && matches!(thread.last(), Message::ToolResults(_)) {
            std::process::exit(super::protocol::CRASH);
        }
    }
}

impl Model for EvalModel {
    async fn generate(&self, thread: &Thread, tools: &[Value]) -> ModelResponse {
        self.boundary(thread);
        self.inner.generate(thread, tools).await
    }

    async fn generate_stream<S: Sink + Send>(
        &self,
        thread: &Thread,
        tools: &[Value],
        sink: &mut S,
    ) -> Result<ModelResponse, S::Error> {
        self.boundary(thread);
        self.inner.generate_stream(thread, tools, sink).await
    }
}
