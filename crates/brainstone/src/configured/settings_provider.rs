/// Where model requests go, in which wire protocol, and how they authenticate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelProvider {
    /// Anthropic's first-party Messages API with an API key.
    Anthropic {
        /// The API key sent as `x-api-key`.
        api_key: String,
    },
    /// `OpenAI`'s first-party Responses API with an API key.
    OpenAi {
        /// The API key sent as a bearer token.
        api_key: String,
    },
    /// Amazon Bedrock. `api_key` is a Bedrock API key (bearer token); without
    /// it the standard AWS credential chain signs requests.
    Bedrock {
        /// Which Bedrock API to call.
        api: BedrockApi,
        /// The AWS region, which fixes the endpoint and the signing scope.
        region: String,
        /// A Bedrock API key, used instead of the AWS credential chain.
        api_key: Option<String>,
        /// Overrides the regional endpoint, for VPC endpoints and tests.
        endpoint_url: Option<String>,
    },
}

/// The Bedrock APIs this runtime can call, each with its own wire protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BedrockApi {
    /// `InvokeModelWithResponseStream` on `bedrock-runtime`.
    Runtime,
    /// Claude in Amazon Bedrock: the Messages API on `bedrock-mantle`.
    Mantle,
    /// The `OpenAI` Responses API on `bedrock-runtime`.
    OpenAiRuntime,
}

/// Each Bedrock API by its `MODEL_PROVIDER` value.
pub(crate) const BEDROCK_APIS: [(&str, BedrockApi); 3] = [
    ("bedrock", BedrockApi::Runtime),
    ("bedrock-mantle", BedrockApi::Mantle),
    ("bedrock-openai", BedrockApi::OpenAiRuntime),
];

impl ModelProvider {
    /// The provider shape without its secret, for logs.
    pub(crate) fn redacted(&self) -> String {
        match self {
            Self::Anthropic { .. } => "anthropic".to_owned(),
            Self::OpenAi { .. } => "openai".to_owned(),
            Self::Bedrock {
                api,
                region,
                api_key,
                ..
            } => {
                let auth = if api_key.is_some() { "bearer" } else { "sigv4" };
                format!("{api:?}({region}, {auth})")
            }
        }
    }

    /// Which model a target serves, without its secret.
    pub(crate) fn named(&self, model: &str) -> String {
        named(&self.target(), model)
    }

    /// Where requests go, without the model: the provider, and on Bedrock its region.
    pub(crate) fn target(&self) -> String {
        match self {
            Self::Anthropic { .. } => "anthropic".to_owned(),
            Self::OpenAi { .. } => "openai".to_owned(),
            Self::Bedrock { api, region, .. } => {
                let (name, _) = BEDROCK_APIS
                    .iter()
                    .find(|(_, candidate)| candidate == api)
                    .expect("every Bedrock API has a provider name");
                format!("{name}/{region}")
            }
        }
    }
}

/// The name stored on every response `model` produces at `target`. Each model
/// is shown only the reasoning stored under its own name, so two targets must
/// never share one.
pub(crate) fn named(target: &str, model: &str) -> String {
    format!("{target}/{model}")
}
