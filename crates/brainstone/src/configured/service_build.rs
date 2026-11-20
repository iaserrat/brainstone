//! Compose the durable agent engine from settings.

use runtime::DurableStore;
use surface::Destinations;

use crate::configured::service_admission::{AgentHost, GatedHost};
use crate::configured::{Error, Settings};

/// A composed agent: durable store, admission chain, host, and MCP clients.
///
/// Drive it yourself: `admission.accept` stores a prompt durably,
/// `host.run` drains accepted prompts until shutdown, and `host.execute`
/// runs one accepted operation in-process. Close with
/// [`Engine::shutdown`].
#[derive(Debug)]
pub struct Engine {
    /// The durable store behind every accepted prompt.
    pub store: DurableStore,
    /// Executes accepted operations.
    pub host: AgentHost,
    /// Admission chain (gates, memory injection, skills, model selection).
    pub admission: GatedHost,
    /// Connected MCP tool servers.
    pub mcp: mcp::ClientSet,
}

impl Engine {
    /// Close admission, wait for claimed work, then close MCP and the database.
    ///
    /// Safe to call again after completion. Closing the pool also closes store
    /// clones held by the caller. Join any spawned `host.run` task afterwards.
    /// Call from the process owner, never from a tool executing on this host.
    pub async fn shutdown(&mut self) {
        self.host.shutdown().await;
        self.mcp.close().await;
        self.store.close().await;
    }
}

/// Compose the configured providers and capabilities without schedule tools.
///
/// No destination implementation is needed. Use [`build`] to enable scheduled
/// posting through your surface's destination resolver.
///
/// # Errors
/// Returns the same configuration and component failures as [`build`].
pub async fn configured(settings: Settings) -> Result<Engine, Error> {
    build(settings, Option::<NoDestinations>::None).await
}

// Uninhabited: no resolver exists when scheduled posting is disabled.
enum NoDestinations {}

impl Destinations for NoDestinations {
    async fn resolve(
        &self,
        _: surface::PostRequest<'_>,
    ) -> Result<runtime::Destination, agent::ToolError> {
        match *self {}
    }
}

/// Compose the engine from `settings`.
///
/// `destinations` decides where a schedule's runs post; without one the
/// schedule tools are not offered.
///
/// # Errors
///
/// Returns stable configuration, database, or model failures.
pub async fn build<D: Destinations + 'static>(
    settings: Settings,
    destinations: Option<D>,
) -> Result<Engine, Error> {
    crate::configured::tls::install();
    let parts = crate::configured::service_core::prepare(&settings.core).await?;
    let store = parts.store.clone();
    let (mcp, base_tools) = mcp::ClientSet::connect(&settings.core.mcp)
        .await
        .map_err(|error| Error::component(error.code(), error))?;
    let tools =
        crate::configured::service_tools::tools(&settings, &store, &base_tools, destinations);
    let host = crate::configured::service_core::host(&settings.core, parts, tools)?;
    crate::configured::service_skills::log_diagnostics(&settings.core.skills);
    let admission = crate::configured::service_admission::chain(&settings.core, &host, &store);
    Ok(Engine {
        store,
        host,
        admission,
        mcp,
    })
}
