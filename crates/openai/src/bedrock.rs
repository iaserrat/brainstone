//! The Responses API on Amazon Bedrock's `bedrock-runtime` endpoint.

use crate::auth::Signer;
use crate::{Auth, ConfigError, ResponsesModel};

/// `SigV4` service name of `bedrock-runtime`.
const RUNTIME_SERVICE: &str = "bedrock";

impl ResponsesModel {
    /// The Responses API on `bedrock-runtime` in `region`, or at `base`, a VPC
    /// or partition endpoint. `model` is an inference profile id.
    ///
    /// # Errors
    ///
    /// Returns `empty_region`, `empty_model`, `empty_bearer`, or
    /// `invalid_endpoint`.
    pub fn bedrock_runtime(
        region: &str,
        base: Option<&str>,
        model: &str,
        auth: Auth,
    ) -> Result<Self, ConfigError> {
        let region = non_empty(region)?;
        let default = format!("https://bedrock-runtime.{region}.amazonaws.com");
        let base = base.unwrap_or(&default).trim_end_matches('/');
        let endpoint = format!("{base}/openai/v1/responses");
        Self::build(&endpoint, model, signer(auth, RUNTIME_SERVICE))
    }
}

fn signer(auth: Auth, service: &'static str) -> Signer {
    match auth {
        Auth::Bearer(token) => Signer::Bearer(token),
        Auth::SigV4(identity) => Signer::SigV4(identity, service),
    }
}

fn non_empty(region: &str) -> Result<&str, ConfigError> {
    let region = region.trim();
    if region.is_empty() {
        return Err(ConfigError::EmptyRegion);
    }
    Ok(region)
}
