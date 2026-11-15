use agent::Thread;
use serde::Serialize;
use serde_json::Value;

use crate::model::AnthropicModel;
use crate::{Effort, Endpoint};

/// Summaries are billed like omitted thinking and replay the same way, and
/// only they give surfaces reasoning progress.
const THINKING_DISPLAY: &str = "summarized";

/// Bedrock runtime carries the API version in the body, not a header.
const BEDROCK_ANTHROPIC_VERSION: &str = "bedrock-2023-05-31";

#[derive(Serialize)]
pub(crate) struct Request<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    anthropic_version: Option<&'static str>,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<Value>,
    tools: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    thinking: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_config: Option<OutputConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

#[derive(Serialize)]
struct OutputConfig {
    effort: Effort,
}

/// Top-level automatic caching with the default 5-minute TTL.
#[derive(Serialize)]
struct CacheControl {
    #[serde(rename = "type")]
    kind: &'static str,
}

impl<'a> Request<'a> {
    pub(crate) fn new(
        model: &'a AnthropicModel,
        selected: &'a str,
        effort: Option<Effort>,
        thread: &'a Thread,
        tools: &'a [Value],
    ) -> Result<Self, crate::failure::Failure> {
        // The runtime names the model in the URL and always streams.
        let runtime = matches!(model.endpoint, Endpoint::BedrockRuntime(_));
        Ok(Self {
            model: (!runtime).then_some(selected),
            anthropic_version: runtime.then_some(BEDROCK_ANTHROPIC_VERSION),
            max_tokens: model.max_tokens,
            system: &thread.system,
            messages: crate::history::messages(thread)?,
            tools: crate::tool_def::tools(tools)?,
            stream: (!runtime).then_some(true),
            thinking: serde_json::json!({"type": "adaptive", "display": THINKING_DISPLAY}),
            output_config: effort.map(|effort| OutputConfig { effort }),
            cache_control: model
                .prompt_cache
                .then_some(CacheControl { kind: "ephemeral" }),
        })
    }
}
