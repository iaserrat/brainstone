use crate::config_raw::Server;
use crate::error::Error;

pub(crate) fn transport(raw: &Server) -> Result<(), Error> {
    match (&raw.command, &raw.url) {
        (Some(_), None) => stdio(raw),
        (None, Some(url)) => http(raw, url),
        _ => failure("each server needs exactly one of command or url"),
    }
}

fn stdio(raw: &Server) -> Result<(), Error> {
    let mixed =
        raw.bearer_token_env.is_some() || !raw.headers.is_empty() || !raw.env_headers.is_empty();
    (!mixed)
        .then_some(())
        .ok_or_else(|| error("stdio server has HTTP-only options"))
}

fn http(raw: &Server, url: &str) -> Result<(), Error> {
    let mixed = !raw.args.is_empty() || !raw.env.is_empty() || raw.cwd.is_some();
    if mixed {
        return failure("HTTP server has stdio-only options");
    }
    let parsed =
        reqwest::Url::parse(url).map_err(|cause| error(format!("invalid MCP URL: {cause}")))?;
    matches!(parsed.scheme(), "http" | "https")
        .then_some(())
        .ok_or_else(|| error("MCP URL must use HTTP or HTTPS"))
}

fn failure(message: &str) -> Result<(), Error> {
    Err(error(message))
}

fn error(message: impl std::fmt::Display) -> Error {
    Error::new("mcp_configuration", message)
}
