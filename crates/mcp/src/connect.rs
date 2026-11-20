use std::sync::Arc;

use rmcp::model::ProtocolVersion;
use rmcp::transport::{
    TransportAdapterIdentity as Identity, streamable_http_client::StreamableHttpError as HttpError,
};
use rmcp::{RoleClient, service::RunningService};
#[path = "connect_serve.rs"]
mod handshake;
use handshake::serve;
pub(crate) use handshake::serve_stream;

use crate::client::Client;
use crate::config::{Server, Transport};
use crate::error::Error;
use crate::handler::Handler;
use crate::session::{Connector, SessionState};

pub(crate) async fn connect(
    server: &Server,
    interaction: Arc<dyn crate::Interaction>,
) -> Result<Client, Error> {
    let owned = server.clone();
    let connector: Connector = Arc::new(move || {
        let server = owned.clone();
        let interaction = interaction.clone();
        Box::pin(async move { service(&server, interaction).await })
    });
    let service = connector().await?;
    Ok(Client {
        name: server.name().to_owned(),
        session: SessionState::new(
            service,
            server.tool_timeout(),
            Some(connector),
            server.access,
        ),
    })
}

async fn service(
    server: &Server,
    interaction: Arc<dyn crate::Interaction>,
) -> Result<RunningService<RoleClient, Handler>, Error> {
    tokio::time::timeout(server.startup_timeout(), negotiate(server, interaction))
        .await
        .map_err(|_| Error::new("mcp_startup_timeout", server.name()))?
}

/// Some servers mishandle a protocol version newer than they support: they
/// issue a session and then reject every request carrying it (GitHub's hosted
/// MCP, 2026-08). Offer newest first and walk down until one is honored.
async fn negotiate(
    server: &Server,
    interaction: Arc<dyn crate::Interaction>,
) -> Result<RunningService<RoleClient, Handler>, Error> {
    let mut failure = None;
    for version in crate::handler::supported_versions() {
        match attempt(server, interaction.clone(), version).await {
            Ok(service) => return Ok(service),
            Err(error) if error.code() == "mcp_configuration" => return Err(error),
            Err(error) => {
                tracing::debug!(server = server.name(), error = %error, "MCP handshake attempt failed");
                failure = Some(error);
            }
        }
    }
    Err(failure.expect("supported_versions is never empty"))
}

async fn attempt(
    server: &Server,
    interaction: Arc<dyn crate::Interaction>,
    version: ProtocolVersion,
) -> Result<RunningService<RoleClient, Handler>, Error> {
    match &server.transport {
        Transport::Stdio {
            command,
            args,
            env,
            cwd,
        } => {
            let transport = crate::connect_transport::stdio(command, args, env, cwd)?;
            serve(server, transport, interaction, version).await
        }
        Transport::Http {
            url,
            bearer_token,
            headers,
        } => {
            let transport = crate::connect_transport::http(url, bearer_token.as_deref(), headers)?;
            serve::<_, HttpError<reqwest::Error>, Identity>(server, transport, interaction, version)
                .await
        }
    }
}
