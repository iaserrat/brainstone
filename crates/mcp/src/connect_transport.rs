use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use reqwest::header::{HeaderName, HeaderValue};
use rmcp::transport::{
    StreamableHttpClientTransport, TokioChildProcess,
    streamable_http_client::StreamableHttpClientTransportConfig,
};
use tokio::process::Command;

use crate::error::Error;

pub(crate) fn stdio(
    program: &str,
    args: &[String],
    env: &BTreeMap<String, String>,
    cwd: &Path,
) -> Result<TokioChildProcess, Error> {
    let mut command = Command::new(program);
    command.args(args).env_clear().envs(env).current_dir(cwd);
    TokioChildProcess::new(command).map_err(|error| Error::new("mcp_connect", error))
}

pub(crate) fn http(
    url: &str,
    bearer_token: Option<&str>,
    headers: &BTreeMap<String, String>,
) -> Result<StreamableHttpClientTransport<reqwest::Client>, Error> {
    let mut config = StreamableHttpClientTransportConfig::with_uri(url.to_owned());
    config.auth_header = bearer_token.map(str::to_owned);
    config.custom_headers = parse_headers(headers)?;
    Ok(StreamableHttpClientTransport::from_config(config))
}

fn parse_headers(
    headers: &BTreeMap<String, String>,
) -> Result<HashMap<HeaderName, HeaderValue>, Error> {
    headers
        .iter()
        .map(|(name, value)| Ok((header_name(name)?, header_value(value)?)))
        .collect()
}

fn header_name(value: &str) -> Result<HeaderName, Error> {
    HeaderName::try_from(value).map_err(|error| Error::new("mcp_configuration", error))
}

fn header_value(value: &str) -> Result<HeaderValue, Error> {
    HeaderValue::try_from(value).map_err(|error| Error::new("mcp_configuration", error))
}
