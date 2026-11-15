//! Compose the durable agent engine from settings.

use runtime::DurableStore;
use surface::Destinations;

use crate::service_admission::{AgentHost, GatedHost};
use crate::{Error, Settings};

/// A composed agent: durable store, admission chain, host, and MCP clients.
///
/// Drive it yourself: `admission.accept` stores a prompt durably,
/// `host.run` drains accepted prompts until shutdown, and `host.execute`
/// runs one accepted operation in-process. Close with
/// `host.shutdown`, then `mcp.close`, then `store.close`.
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
    crate::tls::install();
    let parts = crate::service_core::prepare(&settings.core).await?;
    let store = parts.store.clone();
    let (mcp, base_tools) = mcp::ClientSet::connect(&settings.core.mcp)
        .await
        .map_err(|error| Error::component(error.code(), error))?;
    let tools = crate::service_tools::tools(&settings, &store, &base_tools, destinations);
    let host = crate::service_core::host(&settings.core, parts, tools)?;
    crate::service_skills::log_diagnostics(&settings.core.skills);
    let admission = crate::service_admission::chain(&settings.core, &host, &store);
    Ok(Engine {
        store,
        host,
        admission,
        mcp,
    })
}
