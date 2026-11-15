use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct File {
    #[serde(default)]
    pub(crate) mcp_servers: BTreeMap<String, Server>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Server {
    pub(crate) command: Option<String>,
    #[serde(default)]
    pub(crate) args: Vec<String>,
    #[serde(default)]
    pub(crate) env: BTreeMap<String, String>,
    pub(crate) cwd: Option<String>,
    pub(crate) url: Option<String>,
    // No OAuth on purpose. If an OAuth-only server ever forces it, the boring
    // subset is token_url + client/refresh env refs with in-process refresh.
    pub(crate) bearer_token_env: Option<String>,
    #[serde(default)]
    pub(crate) headers: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) env_headers: BTreeMap<String, String>,
    #[serde(default = "yes")]
    pub(crate) enabled: bool,
    #[serde(default = "yes")]
    pub(crate) required: bool,
    #[serde(default = "startup_timeout")]
    pub(crate) startup_timeout_seconds: u64,
    #[serde(default = "tool_timeout")]
    pub(crate) tool_timeout_seconds: u64,
    #[serde(default)]
    pub(crate) access: agent::AccessPolicy,
    pub(crate) enabled_tools: Option<Vec<String>>,
}

const fn yes() -> bool {
    true
}

const fn startup_timeout() -> u64 {
    10
}

const fn tool_timeout() -> u64 {
    60
}
