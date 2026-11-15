use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{config_build, error::Error};

const CONFIG_PATH: &str = "mcp.toml";

/// The operator's `mcp.toml`: the MCP servers this process offers.
#[derive(Clone, Default)]
pub struct Config {
    servers: Vec<Server>,
}

#[derive(Clone)]
/// One configured server: its transport, timeouts, and access policy.
pub struct Server {
    pub(crate) name: String,
    pub(crate) transport: Transport,
    pub(crate) required: bool,
    pub(crate) startup_timeout: Duration,
    pub(crate) tool_timeout: Duration,
    pub(crate) access: agent::AccessPolicy,
    pub(crate) enabled_tools: Option<Vec<String>>,
}

#[derive(Clone)]
/// How the client reaches one server.
pub enum Transport {
    /// A child process speaking JSON-RPC over stdin and stdout.
    Stdio {
        /// The executable to spawn.
        command: String,
        /// Arguments passed to it, exactly as configured.
        args: Vec<String>,
        /// Variables added to the child's inherited environment.
        env: BTreeMap<String, String>,
        /// The child's working directory.
        cwd: PathBuf,
    },
    /// A remote server reached over Streamable HTTP.
    Http {
        /// The endpoint URL.
        url: String,
        /// A bearer token sent as authorization, when the server requires one.
        bearer_token: Option<String>,
        /// Additional request headers, such as a proxy's identity header.
        headers: BTreeMap<String, String>,
    },
}

impl Config {
    /// Load `mcp.toml`; a missing file means no MCP servers are configured.
    ///
    /// # Errors
    ///
    /// Returns `mcp_configuration` for unreadable, invalid, or unresolved input.
    pub fn load(root: &Path, environment: &BTreeMap<String, String>) -> Result<Self, Error> {
        let path = root.join(CONFIG_PATH);
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(Error::new("mcp_configuration", error)),
        };
        config_build::build(&source, root, environment)
    }

    #[must_use]
    /// Every configured server, in file order.
    pub fn servers(&self) -> &[Server] {
        &self.servers
    }

    pub(crate) fn new(servers: Vec<Server>) -> Self {
        Self { servers }
    }
}

impl Server {
    #[must_use]
    /// The stable name tool prefixes and log lines use.
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    /// Whether startup fails when this server cannot connect.
    pub const fn required(&self) -> bool {
        self.required
    }

    #[must_use]
    /// How this server is reached.
    pub const fn transport(&self) -> &Transport {
        &self.transport
    }

    #[must_use]
    /// How long startup may take before the server counts as unreachable.
    pub const fn startup_timeout(&self) -> Duration {
        self.startup_timeout
    }

    #[must_use]
    /// The deadline one tool call on this server gets.
    pub const fn tool_timeout(&self) -> Duration {
        self.tool_timeout
    }

    #[must_use]
    /// The allowlist of tool names, or `None` for every tool the server lists.
    pub fn enabled_tools(&self) -> Option<&[String]> {
        self.enabled_tools.as_deref()
    }
}
