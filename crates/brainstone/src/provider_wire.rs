/// The wire protocol its provider speaks.
#[derive(Clone)]
pub(crate) enum Wire {
    Anthropic(anthropic::AnthropicModel),
    OpenAi(openai::ResponsesModel),
    Compatible(agent::CapabilityModel<openai::ResponsesModel>),
    Chat(agent::CapabilityModel<openai::ChatCompletionsModel>),
}
