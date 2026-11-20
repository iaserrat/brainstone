use std::sync::Arc;
use std::time::Duration;

use agent::Tool;
use rmcp::{RoleClient, transport::IntoTransport};

use crate::session::{Session, SessionState};
use crate::{Interaction, error::Error, interaction::Decline, tool::RemoteTool};

/// One connected server: its stable name and live session.
pub struct Client {
    pub(crate) name: String,
    pub(crate) session: Session,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Client")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Connect an arbitrary MCP byte transport. Production callers normally
    /// use [`ClientSet`](crate::ClientSet) with configured HTTP or stdio.
    ///
    /// # Errors
    ///
    /// Returns `mcp_connect` when protocol negotiation or transport startup fails.
    pub async fn connect_stream<T, E, A>(name: &str, transport: T) -> Result<Self, Error>
    where
        T: IntoTransport<RoleClient, E, A>,
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::connect_stream_with_timeout(
            name,
            transport,
            Duration::from_mins(1),
            Arc::new(Decline),
        )
        .await
    }

    pub(crate) async fn connect_stream_with_timeout<T, E, A>(
        name: &str,
        transport: T,
        tool_timeout: Duration,
        interaction: Arc<dyn Interaction>,
    ) -> Result<Self, Error>
    where
        T: IntoTransport<RoleClient, E, A>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let service = crate::connect::serve_stream(name, transport, interaction).await?;
        Ok(Self {
            name: name.to_owned(),
            session: SessionState::new(service, tool_timeout, None, agent::AccessPolicy::default()),
        })
    }

    /// Discover every remote tool and adapt it to the agent tool seam.
    ///
    /// # Errors
    ///
    /// Returns a stable connection, discovery, or invalid-name error.
    pub async fn tools(&self) -> Result<Vec<Arc<dyn Tool>>, Error> {
        discover(&self.name, &self.session).await
    }

    /// Close the protocol service and its transport, including a stdio child.
    ///
    /// # Errors
    ///
    /// Returns `mcp_shutdown` if the service cleanup task cannot be joined.
    pub async fn close(&mut self) -> Result<(), Error> {
        let service = self.session.service.write().await.take();
        let Some(mut service) = service else {
            return Ok(());
        };
        service
            .close()
            .await
            .map_err(|error| Error::new("mcp_shutdown", error))?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn session(&self) -> Session {
        self.session.clone()
    }
}

pub(crate) async fn discover(server: &str, session: &Session) -> Result<Vec<Arc<dyn Tool>>, Error> {
    let tools = session
        .peer()
        .await?
        .list_all_tools()
        .await
        .map_err(|error| Error::new("mcp_list_tools", error))?;
    tools
        .into_iter()
        .map(|tool| RemoteTool::new(server, tool, session.clone()))
        .map(|tool| tool.map(|tool| -> Arc<dyn Tool> { Arc::new(tool) }))
        .collect()
}
