use anthropic::{Auth, Effort, Endpoint};

use crate::model_fallback::Fallback;
use crate::provider_model::{ProviderModel, Wire};
use crate::{CoreSettings, Error, ModelProvider};

#[path = "service_model_bedrock.rs"]
mod bedrock;
#[path = "service_model_wire.rs"]
mod constructors;

const ANTHROPIC_MESSAGES: &str = "https://api.anthropic.com/v1/messages";

/// The model every agent and subagent of this process calls.
pub(crate) type FailoverModel = Fallback<ProviderModel, ProviderModel>;

/// One model as configured: its id, output cap, reasoning effort, and caching.
struct Spec<'a> {
    model: &'a str,
    max_tokens: u32,
    effort: Effort,
    prompt_cache: bool,
}

pub(crate) async fn build(settings: &CoreSettings) -> Result<FailoverModel, Error> {
    let primary = primary(settings).await?;
    let Some(fallback) = &settings.fallback else {
        return Ok(Fallback::single(primary));
    };
    let spec = Spec {
        model: &fallback.model,
        max_tokens: fallback.max_tokens,
        effort: fallback.effort,
        prompt_cache: settings.prompt_cache,
    };
    let secondary = configured(
        &fallback.provider,
        &spec,
        settings.targets.fallback.as_ref(),
    )
    .await?;
    Ok(Fallback::new(primary, secondary))
}

async fn primary(settings: &CoreSettings) -> Result<ProviderModel, Error> {
    let spec = Spec {
        model: &settings.anthropic_model,
        max_tokens: settings.anthropic_max_tokens,
        effort: settings.model_effort,
        prompt_cache: settings.prompt_cache,
    };
    let primary = configured(
        &settings.model_provider,
        &spec,
        settings.targets.primary.as_ref(),
    )
    .await?;
    Ok(primary)
}

async fn configured(
    provider: &ModelProvider,
    spec: &Spec<'_>,
    options: Option<&crate::target_options::TargetOptions>,
) -> Result<ProviderModel, Error> {
    if let (Some(options), ModelProvider::OpenAi { api_key }) = (options, provider) {
        return crate::service_model_compatible::build(options, spec.model, api_key);
    }
    Ok(ProviderModel::new(
        provider.target(),
        spec.model,
        wire(provider, spec).await?,
    ))
}

async fn wire(provider: &ModelProvider, spec: &Spec<'_>) -> Result<Wire, Error> {
    match provider {
        ModelProvider::Anthropic { api_key } => {
            let endpoint =
                Endpoint::messages(ANTHROPIC_MESSAGES).map_err(constructors::component)?;
            constructors::messages(endpoint, Auth::ApiKey(api_key.clone()), spec)
        }
        ModelProvider::OpenAi { api_key } => constructors::responses(
            openai::ResponsesModel::new(openai::OPENAI_RESPONSES_URL, spec.model, api_key),
            spec,
        ),
        ModelProvider::Bedrock {
            api,
            region,
            api_key,
            endpoint_url,
        } => {
            let aws = bedrock::Aws::load(region, endpoint_url.as_deref(), api_key.as_ref()).await?;
            bedrock::wire(*api, aws, spec)
        }
    }
}
