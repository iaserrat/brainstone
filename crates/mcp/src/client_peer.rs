use rmcp::RoleClient;

use crate::{Client, Error};

impl Client {
    #[must_use]
    /// The stable server name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Clone the official SDK peer, exposing resources, prompts, completion,
    /// subscriptions, notifications, tasks, custom methods, and raw requests.
    ///
    /// # Errors
    ///
    /// Returns `mcp_closed` after this client has shut down.
    pub async fn peer(&self) -> Result<rmcp::Peer<RoleClient>, Error> {
        self.session.peer().await
    }
}
