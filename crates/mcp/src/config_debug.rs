//! How a configured server is reported: identity, timeouts, and policy, never
//! a token, a header value, or an environment value.

use crate::config::{Config, Server, Transport};

impl std::fmt::Debug for Config {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.servers().iter().map(Server::name).collect();
        formatter
            .debug_struct("Config")
            .field("servers", &names)
            .finish()
    }
}

/// Names, timeouts, and policy; never tokens, headers, or environment values.
impl std::fmt::Debug for Server {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Server")
            .field("name", &self.name)
            .field("transport", &self.transport)
            .field("required", &self.required)
            .field("startup_timeout", &self.startup_timeout)
            .field("tool_timeout", &self.tool_timeout)
            .field("access", &self.access)
            .field("enabled_tools", &self.enabled_tools)
            .finish()
    }
}

/// The transport kind alone: a URL can carry a token, an environment a secret.
impl std::fmt::Debug for Transport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple(match self {
                Self::Stdio { .. } => "Stdio",
                Self::Http { .. } => "Http",
            })
            .finish()
    }
}
