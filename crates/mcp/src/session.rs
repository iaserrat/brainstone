use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use rmcp::{Peer, RoleClient, service::RunningService};
use tokio::sync::{Mutex, RwLock};

use crate::error::Error;
use crate::handler::Handler;

pub(crate) type Session = Arc<SessionState>;

type Service = RunningService<RoleClient, Handler>;
type ConnectFuture = Pin<Box<dyn Future<Output = Result<Service, Error>> + Send>>;

/// Re-establishes a dead transport; `None` disables self-healing.
pub(crate) type Connector = Arc<dyn Fn() -> ConnectFuture + Send + Sync>;

pub(crate) struct SessionState {
    pub(crate) service: RwLock<Option<Service>>,
    pub(crate) tool_timeout: Duration,
    pub(crate) access: agent::AccessPolicy,
    pub(crate) call: Mutex<()>,
    pub(crate) invalidated: std::sync::atomic::AtomicBool,
    connector: Option<Connector>,
}

impl SessionState {
    pub(crate) fn new(
        service: Service,
        tool_timeout: Duration,
        connector: Option<Connector>,
        access: agent::AccessPolicy,
    ) -> Session {
        Arc::new(Self {
            service: RwLock::new(Some(service)),
            tool_timeout,
            access,
            call: Mutex::new(()),
            invalidated: std::sync::atomic::AtomicBool::new(false),
            connector,
        })
    }

    /// The protocol peer of the current connection.
    ///
    /// # Errors
    ///
    /// Returns `mcp_closed` after the session was deliberately closed.
    pub(crate) async fn peer(&self) -> Result<Peer<RoleClient>, Error> {
        let guard = self.service.read().await;
        let service = guard
            .as_ref()
            .ok_or_else(|| Error::new("mcp_closed", "connection is closed"))?;
        Ok(service.peer().clone())
    }

    /// Swap a dead transport for a fresh connection before a tool call.
    /// Callers hold the exclusive `call` lock, so at most one reconnect runs.
    /// A deliberately closed session (`None`) stays closed.
    pub(crate) async fn ensure_live(&self) -> Result<(), Error> {
        let dead = match self.service.read().await.as_ref() {
            Some(service) => {
                service.is_transport_closed()
                    || self.invalidated.load(std::sync::atomic::Ordering::Acquire)
            }
            None => false,
        };
        if !dead {
            return Ok(());
        }
        let Some(connector) = &self.connector else {
            return Ok(());
        };
        let mut guard = self.service.write().await;
        cleanup(guard.as_mut()).await?;
        let service = connector().await?;
        *guard = Some(service);
        self.invalidated
            .store(false, std::sync::atomic::Ordering::Release);
        Ok(())
    }
}

async fn cleanup(service: Option<&mut Service>) -> Result<(), Error> {
    if let Some(service) = service {
        service
            .close_with_timeout(Duration::from_secs(5))
            .await
            .map_err(|_| Error::new("mcp_shutdown", "connection cleanup failed"))?
            .ok_or_else(|| Error::new("mcp_shutdown", "connection cleanup timed out"))?;
    }
    Ok(())
}
