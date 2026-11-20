use std::time::SystemTime;

use aws_sigv4::http_request::{SignableBody, SignableRequest, SigningSettings, sign};
use aws_sigv4::sign::v4;

use crate::{Error, SigV4};

impl SigV4 {
    /// Headers that sign one POST of `body` to `url` for AWS `service`. The
    /// signature covers `base`, so callers must send exactly those alongside.
    ///
    /// # Errors
    ///
    /// Returns `credentials_unavailable` when the chain yields no identity and
    /// `signing_failed` when the request cannot be signed.
    pub async fn headers(
        &self,
        service: &str,
        url: &str,
        base: &[(&str, &str)],
        body: &[u8],
    ) -> Result<Vec<(String, String)>, Error> {
        let identity = self.credentials().await?.into();
        let params = v4::SigningParams::builder()
            .identity(&identity)
            .region(&self.region)
            .name(service)
            .time(SystemTime::now())
            .settings(SigningSettings::default())
            .build()
            .map_err(signing)?;
        let request =
            SignableRequest::new("POST", url, base.iter().copied(), SignableBody::Bytes(body))
                .map_err(signing)?;
        let (instructions, _signature) =
            sign(request, &params.into()).map_err(signing)?.into_parts();
        Ok(instructions
            .headers()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect())
    }
}

fn signing(error: impl std::fmt::Display) -> Error {
    Error::Signing(error.to_string())
}
