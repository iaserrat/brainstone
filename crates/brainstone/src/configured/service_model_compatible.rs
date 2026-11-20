use crate::configured::{
    Error,
    provider_model::{ProviderModel, Wire},
    target_options::{Protocol, TargetOptions},
};
use agent::{CapabilityModel, Model};
use openai::{ChatCompletionsModel, ResponsesModel};

pub(crate) fn build(
    options: &TargetOptions,
    model: &str,
    key: &str,
) -> Result<ProviderModel, Error> {
    let wire = match options.protocol {
        Protocol::Responses => Wire::Compatible(responses(options, model, key)?),
        Protocol::ChatCompletions => Wire::Chat(chat(options, model, key)?),
    };
    Ok(ProviderModel::new(options.target.clone(), model, wire))
}

fn responses(
    options: &TargetOptions,
    model: &str,
    key: &str,
) -> Result<CapabilityModel<ResponsesModel>, Error> {
    let model = if key.is_empty() {
        ResponsesModel::unauthenticated(&options.endpoint, model)
    } else {
        ResponsesModel::new(&options.endpoint, model, key)
    }
    .map_err(component)?;
    let mut model = model
        .with_parameters(options.parameters.clone())
        .with_headers(options.headers.clone())
        .with_idle_timeout(options.idle)
        .map_err(component)?;
    if let Some(effort) = options.effort {
        model = model.with_effort(effort);
    }
    if let Some(limit) = options.output {
        model = model.with_max_output_tokens(limit);
    }
    if options.reasoning_summary {
        model = model.with_reasoning_summary();
    }
    Ok(capabilities(model, options))
}

fn chat(
    options: &TargetOptions,
    model: &str,
    key: &str,
) -> Result<CapabilityModel<ChatCompletionsModel>, Error> {
    let model = if key.is_empty() {
        ChatCompletionsModel::unauthenticated(&options.endpoint, model)
    } else {
        ChatCompletionsModel::new(&options.endpoint, model, key)
    }
    .map_err(component)?;
    let mut model = model
        .with_parameters(options.parameters.clone())
        .with_headers(options.headers.clone())
        .with_idle_timeout(options.idle)
        .map_err(component)?;
    if let Some(effort) = options.effort {
        model = model.with_effort(effort);
    }
    if let Some(limit) = options.output {
        model = model.with_max_output_tokens(limit);
    }
    Ok(capabilities(model, options))
}

fn capabilities<M: Model>(model: M, options: &TargetOptions) -> CapabilityModel<M> {
    let model = CapabilityModel::new(model, options.capabilities);
    match options.context {
        Some(context) => model.with_context_window(context),
        None => model,
    }
}

fn component(error: openai::ConfigError) -> Error {
    Error::component(error.code(), error)
}
