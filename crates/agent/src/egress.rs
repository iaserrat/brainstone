//! Where an outbound connection leaves the host.

/// The route a model adapter takes to its provider.
///
/// A proxy names one HTTP endpoint that every request is handed to, and the
/// proxy resolves the provider's name: the host's own route and resolver are
/// never used for provider traffic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Egress {
    /// Over the host's own route.
    #[default]
    Direct,
    /// Through the HTTP proxy at this URL, for example `http://127.0.0.1:8118`.
    Proxy(String),
}

impl Egress {
    /// Route every request through the proxy at this URL.
    #[must_use]
    pub fn proxy(url: impl Into<String>) -> Self {
        Self::Proxy(url.into().trim().to_owned())
    }

    /// The proxy URL this egress names, or `None` for a direct route.
    #[must_use]
    pub fn proxy_url(&self) -> Option<&str> {
        match self {
            Self::Direct => None,
            Self::Proxy(url) => Some(url),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Egress;

    #[test]
    fn a_direct_route_names_no_proxy_and_a_proxy_names_its_trimmed_url() {
        assert_eq!(Egress::Direct.proxy_url(), None);
        assert_eq!(
            Egress::proxy(" http://127.0.0.1:8118 ").proxy_url(),
            Some("http://127.0.0.1:8118")
        );
    }
}
