use agent::{Model, ModelResponse, Sink, Thread};
use serde_json::Value;

use crate::settings_provider::named;

/// One configured model. Every response it produces carries its name, and it
/// is shown only the reasoning stored under that name.
#[derive(Clone)]
pub struct ProviderModel {
    /// [`ModelProvider::target`](crate::ModelProvider::target) of its provider.
    target: String,
    /// The configured model id; a profile's pinned model id replaces it.
    model: String,
    wire: Wire,
}

pub(crate) use crate::provider_wire::Wire;

impl std::fmt::Debug for ProviderModel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderModel")
            .field("target", &self.target)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl ProviderModel {
    pub(crate) fn new(target: String, model: &str, wire: Wire) -> Self {
        Self {
            target,
            model: model.to_owned(),
            wire,
        }
    }

    /// The name of the model that answers `thread`.
    fn producer(&self, thread: &Thread) -> String {
        named(
            &self.target,
            thread.profile().model().unwrap_or(&self.model),
        )
    }
}

impl Model for ProviderModel {
    fn validate_request(
        &self,
        thread: &Thread,
        tools: &[Value],
    ) -> Result<(), agent::CapabilityError> {
        let shown = thread.replay(&self.producer(thread));
        match &self.wire {
            Wire::Anthropic(model) => model.validate_request(&shown, tools),
            Wire::OpenAi(model) => model.validate_request(&shown, tools),
            Wire::Compatible(model) => model.validate_request(&shown, tools),
            Wire::Chat(model) => model.validate_request(&shown, tools),
        }
    }

    fn context_window(&self) -> Option<usize> {
        match &self.wire {
            Wire::Anthropic(model) => model.context_window(),
            Wire::OpenAi(model) => model.context_window(),
            Wire::Compatible(model) => model.context_window(),
            Wire::Chat(model) => model.context_window(),
        }
    }

    async fn generate(&self, thread: &Thread, tools: &[Value]) -> ModelResponse {
        let producer = self.producer(thread);
        let shown = thread.replay(&producer);
        let response = match &self.wire {
            Wire::Anthropic(model) => model.generate(&shown, tools).await,
            Wire::OpenAi(model) => model.generate(&shown, tools).await,
            Wire::Compatible(model) => model.generate(&shown, tools).await,
            Wire::Chat(model) => model.generate(&shown, tools).await,
        };
        named_by(response, producer)
    }

    async fn generate_stream<S: Sink + Send>(
        &self,
        thread: &Thread,
        tools: &[Value],
        sink: &mut S,
    ) -> Result<ModelResponse, S::Error> {
        let producer = self.producer(thread);
        let shown = thread.replay(&producer);
        let response = match &self.wire {
            Wire::Anthropic(model) => model.generate_stream(&shown, tools, sink).await?,
            Wire::OpenAi(model) => model.generate_stream(&shown, tools, sink).await?,
            Wire::Compatible(model) => model.generate_stream(&shown, tools, sink).await?,
            Wire::Chat(model) => model.generate_stream(&shown, tools, sink).await?,
        };
        Ok(named_by(response, producer))
    }
}

fn named_by(response: ModelResponse, producer: String) -> ModelResponse {
    ModelResponse {
        model: Some(producer),
        ..response
    }
}
