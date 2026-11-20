//! AWS identity for Bedrock: the standard credential chain plus a cache.

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use aws_credential_types::Credentials;
use aws_credential_types::provider::{ProvideCredentials, SharedCredentialsProvider};

use crate::Error;

/// Refresh expiring credentials this long before AWS stops accepting them.
const REFRESH_MARGIN: Duration = Duration::from_mins(5);

#[derive(Clone)]
/// AWS identity for one region: the configured credential chain, with the
/// resolved credentials cached until shortly before they expire.
pub struct SigV4 {
    pub(crate) region: String,
    provider: SharedCredentialsProvider,
    // Shared by every lane on this model: temporary credentials are resolved
    // once and refreshed near expiry, not fetched from STS or IMDS per request.
    cached: Arc<Mutex<Option<Credentials>>>,
}

impl SigV4 {
    /// Resolves the standard AWS credential chain (environment, profile, SSO,
    /// web identity, container and instance roles) for `region`.
    ///
    /// # Errors
    ///
    /// Returns `empty_region` for a blank region and `missing_credentials` when
    /// no credential source is configured.
    pub async fn load(region: &str) -> Result<Self, Error> {
        if region.trim().is_empty() {
            return Err(Error::EmptyRegion);
        }
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region.to_owned()))
            .load()
            .await;
        let provider = config
            .credentials_provider()
            .ok_or(Error::MissingCredentials)?;
        Ok(Self::with_provider(region, provider))
    }

    #[must_use]
    /// Uses `provider` instead of loading the chain, for hosts and tests that
    /// supply credentials themselves.
    pub fn with_provider(region: &str, provider: SharedCredentialsProvider) -> Self {
        Self {
            region: region.to_owned(),
            provider,
            cached: Arc::new(Mutex::new(None)),
        }
    }

    /// The credential chain, for clients that sign through the AWS SDK.
    #[must_use]
    pub const fn provider(&self) -> &SharedCredentialsProvider {
        &self.provider
    }

    pub(crate) async fn credentials(&self) -> Result<Credentials, Error> {
        if let Some(credentials) = self.fresh() {
            return Ok(credentials);
        }
        let credentials = self
            .provider
            .provide_credentials()
            .await
            .map_err(|error| Error::Credentials(error.to_string()))?;
        *self.cached.lock().unwrap_or_else(PoisonError::into_inner) = Some(credentials.clone());
        Ok(credentials)
    }

    fn fresh(&self) -> Option<Credentials> {
        let guard = self.cached.lock().unwrap_or_else(PoisonError::into_inner);
        let credentials = guard.as_ref()?;
        let usable_until = SystemTime::now() + REFRESH_MARGIN;
        match credentials.expiry() {
            Some(expiry) if expiry <= usable_until => None,
            _ => Some(credentials.clone()),
        }
    }
}

impl fmt::Debug for SigV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SigV4")
            .field("region", &self.region)
            .finish_non_exhaustive()
    }
}
