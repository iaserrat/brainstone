use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use crate::config::{Config, Server, Transport};
use crate::config_env;
use crate::config_raw;
use crate::config_validate;
use crate::error::Error;

pub(crate) fn build(
    source: &str,
    root: &Path,
    environment: &BTreeMap<String, String>,
) -> Result<Config, Error> {
    let file: config_raw::File =
        toml::from_str(source).map_err(|error| Error::new("mcp_configuration", error))?;
    let servers = file
        .mcp_servers
        .into_iter()
        .filter(|(_, raw)| raw.enabled)
        .map(|(name, raw)| server(name, raw, root, environment))
        .collect::<Result<_, _>>()?;
    Ok(Config::new(servers))
}

fn server(
    name: String,
    raw: config_raw::Server,
    root: &Path,
    environment: &BTreeMap<String, String>,
) -> Result<Server, Error> {
    validate_name(&name)?;
    validate_timeout(raw.startup_timeout_seconds)?;
    validate_timeout(raw.tool_timeout_seconds)?;
    let transport = transport(&raw, root, environment)?;
    Ok(Server {
        name,
        transport,
        required: raw.required,
        startup_timeout: Duration::from_secs(raw.startup_timeout_seconds),
        tool_timeout: Duration::from_secs(raw.tool_timeout_seconds),
        enabled_tools: raw.enabled_tools,
        access: raw.access,
    })
}

fn transport(
    raw: &config_raw::Server,
    root: &Path,
    environment: &BTreeMap<String, String>,
) -> Result<Transport, Error> {
    config_validate::transport(raw)?;
    match (&raw.command, &raw.url) {
        (Some(command), None) => Ok(Transport::Stdio {
            command: command.clone(),
            args: raw.args.clone(),
            env: config_env::child(&raw.env, environment)?,
            cwd: root.join(raw.cwd.as_deref().unwrap_or(".")),
        }),
        (None, Some(url)) => Ok(Transport::Http {
            url: url.clone(),
            bearer_token: config_env::resolve_optional(
                raw.bearer_token_env.as_deref(),
                environment,
            )?,
            headers: http_headers(raw, environment)?,
        }),
        _ => unreachable!("transport shape was validated"),
    }
}

fn http_headers(
    raw: &config_raw::Server,
    environment: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, Error> {
    let mut headers = raw.headers.clone();
    headers.extend(config_env::resolve_map(&raw.env_headers, environment)?);
    Ok(headers)
}

fn validate_name(name: &str) -> Result<(), Error> {
    let valid = !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
    valid
        .then_some(())
        .ok_or_else(|| Error::new("mcp_configuration", format!("invalid server name {name}")))
}

fn validate_timeout(seconds: u64) -> Result<(), Error> {
    (seconds > 0)
        .then_some(())
        .ok_or_else(|| Error::new("mcp_configuration", "timeouts must be positive"))
}
