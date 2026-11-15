use anthropic::Endpoint;
use openai::ResponsesModel;
use sigv4::SigV4;

use super::{
    Spec,
    constructors::{component, messages, responses},
};
use crate::Error;
use crate::provider_model::Wire;
use crate::settings_provider::BedrockApi;

/// Where one Bedrock model is served and how its requests authenticate.
pub(super) struct Aws<'a> {
    pub(super) region: &'a str,
    pub(super) endpoint: Option<&'a str>,
    credential: Credential,
}

/// A Bedrock API key as a bearer token, else the AWS credential chain.
enum Credential {
    Key(String),
    SigV4(SigV4),
}

impl<'a> Aws<'a> {
    pub(super) async fn load(
        region: &'a str,
        endpoint: Option<&'a str>,
        api_key: Option<&String>,
    ) -> Result<Self, Error> {
        let credential = match api_key {
            Some(key) => Credential::Key(key.clone()),
            None => Credential::SigV4(
                SigV4::load(region)
                    .await
                    .map_err(|error| Error::component(error.code(), error))?,
            ),
        };
        Ok(Self {
            region,
            endpoint,
            credential,
        })
    }
}

#[allow(clippy::too_many_lines, reason = "flat dispatch: one arm per variant")]
pub(super) fn wire(api: BedrockApi, aws: Aws<'_>, spec: &Spec<'_>) -> Result<Wire, Error> {
    let Aws {
        region,
        endpoint,
        credential,
    } = aws;
    match api {
        BedrockApi::Runtime => {
            let auth = anthropic_auth(credential);
            let endpoint = Endpoint::bedrock_runtime(region, endpoint, &auth).map_err(component)?;
            messages(endpoint, auth, spec)
        }
        BedrockApi::Mantle => {
            let endpoint = Endpoint::bedrock_mantle(region).map_err(component)?;
            messages(endpoint, anthropic_auth(credential), spec)
        }
        BedrockApi::OpenAiRuntime => {
            let auth = openai_auth(credential);
            responses(
                ResponsesModel::bedrock_runtime(region, endpoint, spec.model, auth),
                spec,
            )
        }
    }
}

fn anthropic_auth(credential: Credential) -> anthropic::Auth {
    match credential {
        Credential::Key(key) => anthropic::Auth::ApiKey(key),
        Credential::SigV4(identity) => anthropic::Auth::SigV4(identity),
    }
}

fn openai_auth(credential: Credential) -> openai::Auth {
    match credential {
        Credential::Key(key) => openai::Auth::Bearer(key),
        Credential::SigV4(identity) => openai::Auth::SigV4(identity),
    }
}
