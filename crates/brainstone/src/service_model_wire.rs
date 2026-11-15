use super::Spec;
use crate::{Error, provider_model::Wire};
use anthropic::{AnthropicModel, Auth, Endpoint};
use openai::ResponsesModel;

pub(super) fn messages(endpoint: Endpoint, auth: Auth, spec: &Spec<'_>) -> Result<Wire, Error> {
    let model = AnthropicModel::new(endpoint, spec.model, auth, spec.max_tokens)
        .map_err(component)?
        .with_effort(spec.effort);
    Ok(Wire::Anthropic(if spec.prompt_cache {
        model.with_prompt_cache()
    } else {
        model
    }))
}

pub(super) fn responses(
    model: Result<ResponsesModel, openai::ConfigError>,
    spec: &Spec<'_>,
) -> Result<Wire, Error> {
    let model = model.map_err(|error| Error::component(error.code(), error))?;
    let effort = crate::provider_effort::openai_effort(spec.effort);
    let model = model
        .with_max_output_tokens(spec.max_tokens)
        .with_effort(effort);
    Ok(Wire::OpenAi(model))
}

pub(super) fn component(error: anthropic::ConfigError) -> Error {
    Error::component(error.code(), error)
}
