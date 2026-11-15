use agent::{Registry, ToolSource};
use std::sync::Arc;

use crate::catalog::{Catalog, ServerTools};
use crate::{Client, Config, Error, Interaction, connect, interaction::Decline};

/// Every configured server, connected at startup and closed together.
#[derive(Default, Debug)]
pub struct ClientSet {
    clients: Vec<Client>,
}

impl ClientSet {
    /// Return a configured client by its stable server name.
    #[must_use]
    pub fn client(&self, name: &str) -> Option<&Client> {
        self.clients.iter().find(|client| client.name() == name)
    }

    /// Connect every enabled server and discover its tools. The registry
    /// lists them again before each claimed operation.
    ///
    /// # Errors
    ///
    /// Returns the first required server failure. Optional failures are logged.
    pub async fn connect(config: &Config) -> Result<(Self, Registry), Error> {
        Self::connect_with_interactions(config, Arc::new(Decline)).await
    }

    /// Connect with the deployment's durable user-interaction implementation.
    ///
    /// # Errors
    ///
    /// Returns the first required server or tool-discovery failure.
    pub async fn connect_with_interactions(
        config: &Config,
        interaction: Arc<dyn Interaction>,
    ) -> Result<(Self, Registry), Error> {
        let mut clients = Vec::new();
        let mut tools = Vec::new();
        for server in config.servers() {
            match start(server, interaction.clone()).await {
                Ok((client, listed)) => {
                    clients.push(client);
                    tools.push(listed);
                }
                Err(error) if !server.required() => {
                    tracing::warn!(server = server.name(), error = %error, "optional MCP server unavailable");
                }
                Err(error) => return Err(error),
            }
        }
        let catalog: Arc<dyn ToolSource> = Arc::new(Catalog::new(tools));
        Ok((Self { clients }, Registry::from_sources(vec![catalog])))
    }

    /// Gracefully terminate every connection and stdio child.
    pub async fn close(&mut self) {
        for client in &mut self.clients {
            if let Err(error) = client.close().await {
                tracing::warn!(error = %error, "MCP client shutdown failed");
            }
        }
    }
}

async fn start(
    server: &crate::Server,
    interaction: Arc<dyn Interaction>,
) -> Result<(Client, ServerTools), Error> {
    let mut client = connect::connect(server, interaction).await?;
    match ServerTools::listed(&client, server.enabled_tools()).await {
        Ok(tools) => Ok((client, tools)),
        Err(error) => {
            client.close().await.ok();
            Err(error)
        }
    }
}
