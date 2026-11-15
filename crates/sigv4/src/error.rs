use std::fmt;

/// Why no signed request could be produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The configured region was blank.
    EmptyRegion,
    /// No credential source is configured for the chain.
    MissingCredentials,
    /// The credential chain failed to resolve an identity at request time.
    Credentials(String),
    /// The signer rejected the request.
    Signing(String),
}

impl Error {
    #[must_use]
    /// The stable code a caller branches on.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EmptyRegion => "empty_region",
            Self::MissingCredentials => "missing_credentials",
            Self::Credentials(_) => "credentials_unavailable",
            Self::Signing(_) => "signing_failed",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRegion => formatter.write_str("AWS region is empty"),
            Self::MissingCredentials => formatter.write_str("no AWS credentials resolved"),
            Self::Credentials(message) | Self::Signing(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Error {}
