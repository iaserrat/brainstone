//! Every server's tools as last listed. The registry lists them again before
//! each claimed operation, so a tool a server adds reaches the next message.

use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use agent::{Tool, ToolSource};

use crate::{Client, Error, session::Session};

/// How long one claim waits for a server's list before keeping the last one.
const LIST_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct ServerTools {
    name: String,
    allowlist: Option<Vec<String>>,
    session: Session,
    tools: RwLock<Vec<Arc<dyn Tool>>>,
}

impl ServerTools {
    /// List `client`'s tools once, keeping only `allowlist` when given.
    pub(crate) async fn listed(
        client: &Client,
        allowlist: Option<&[String]>,
    ) -> Result<Self, Error> {
        let server = Self {
            name: client.name.clone(),
            allowlist: allowlist.map(<[String]>::to_vec),
            session: client.session.clone(),
            tools: RwLock::default(),
        };
        let tools = server.list().await?;
        *server.tools.write().expect("MCP tool list poisoned") = tools;
        Ok(server)
    }

    /// Reconnects a dead transport first, unless a call holds the session:
    /// that call reconnects it, and a busy server is a live one.
    async fn list(&self) -> Result<Vec<Arc<dyn Tool>>, Error> {
        if let Ok(_idle) = self.session.call.try_lock() {
            self.session.ensure_live().await?;
        }
        let tools = crate::client::discover(&self.name, &self.session).await?;
        Ok(tools
            .into_iter()
            .filter(|tool| self.allowed(tool))
            .collect())
    }

    async fn refresh(&self) {
        match tokio::time::timeout(LIST_TIMEOUT, self.list()).await {
            Ok(Ok(tools)) => *self.tools.write().expect("MCP tool list poisoned") = tools,
            Ok(Err(error)) => {
                tracing::warn!(server = %self.name, error = %error, "MCP tool listing failed; keeping the previous tools");
            }
            Err(_) => {
                tracing::warn!(server = %self.name, "MCP tool listing timed out; keeping the previous tools");
            }
        }
    }

    fn allowed(&self, tool: &Arc<dyn Tool>) -> bool {
        let prefix = format!("mcp__{}__", self.name);
        let remote = tool.name().strip_prefix(&prefix).unwrap_or(tool.name());
        let allowlist = self.allowlist.as_deref();
        allowlist.is_none_or(|names| names.iter().any(|name| name == remote))
    }
}

pub(crate) struct Catalog(Vec<ServerTools>);

impl Catalog {
    pub(crate) const fn new(servers: Vec<ServerTools>) -> Self {
        Self(servers)
    }
}

impl ToolSource for Catalog {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        let lists = self
            .0
            .iter()
            .map(|server| server.tools.read().expect("MCP tool list poisoned").clone());
        lists.flatten().collect()
    }

    /// Lists every server at once, so the slowest one bounds the wait.
    fn refresh(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let listings = self.0.iter().map(ServerTools::refresh);
        Box::pin(async move {
            futures_util::future::join_all(listings).await;
        })
    }
}
