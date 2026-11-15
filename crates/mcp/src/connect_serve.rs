use crate::{config::Server, error::Error, handler::Handler};
use rmcp::{
    ClientLifecycleMode, RoleClient, model::ProtocolVersion, serve_client_with_lifecycle,
    service::RunningService, transport::IntoTransport,
};
use std::sync::Arc;
use std::time::Duration;

/// The longest a discover attempt waits for an answer: rmcp's own bound for
/// a peer that never answers `server/discover`, which some legacy servers do.
const MAX_DISCOVER_WAIT: Duration = Duration::from_secs(10);

/// One handshake at `version`. A revision without `initialize` (2026-07-28
/// on) starts with `server/discover`: offering it inside `initialize` lets
/// the server answer with an older revision, which loses MRTR elicitation.
/// A legacy server rejects or never answers discover, and the caller walks
/// down a revision.
pub(super) async fn serve<T, E, A>(
    server: &Server,
    transport: T,
    interaction: Arc<dyn crate::Interaction>,
    version: ProtocolVersion,
) -> Result<RunningService<RoleClient, Handler>, Error>
where
    T: IntoTransport<RoleClient, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let legacy = version.has_initialize();
    let mut handler = Handler::new(server.name(), interaction);
    handler.protocol_version = version.clone();
    if legacy {
        let handshake =
            serve_client_with_lifecycle(handler, transport, ClientLifecycleMode::Initialize);
        return handshake.await.map_err(connected);
    }
    let lifecycle = ClientLifecycleMode::Discover {
        preferred_versions: vec![version],
    };
    let handshake = serve_client_with_lifecycle(handler, transport, lifecycle);
    // Half the startup budget at most, so the `initialize` walk keeps time.
    let wait = MAX_DISCOVER_WAIT.min(server.startup_timeout() / 2);
    let handshake = tokio::time::timeout(wait, handshake).await;
    handshake
        .map_err(|_| Error::new("mcp_connect", "no answer to server/discover"))?
        .map_err(connected)
}

/// One handshake on a stream that cannot be reopened at an older revision:
/// discover, then its fallback to `initialize`, both run on it.
pub(crate) async fn serve_stream<T, E, A>(
    name: &str,
    transport: T,
    interaction: Arc<dyn crate::Interaction>,
) -> Result<RunningService<RoleClient, Handler>, Error>
where
    T: IntoTransport<RoleClient, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let lifecycle = ClientLifecycleMode::Auto {
        preferred_versions: crate::handler::supported_versions(),
        legacy_version: Some(ProtocolVersion::LATEST_WITH_INITIALIZE),
    };
    let handler = Handler::new(name, interaction);
    let handshake = serve_client_with_lifecycle(handler, transport, lifecycle).await;
    handshake.map_err(connected)
}

fn connected(error: impl std::fmt::Display) -> Error {
    Error::new("mcp_connect", error)
}
