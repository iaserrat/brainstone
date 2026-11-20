use std::sync::{Arc, Mutex};

use rmcp::{
    ClientHandler,
    model::{
        ClientConfig, ElicitRequestParams, ElicitResult, ElicitationCapability,
        FormElicitationCapability, JsonObject, ProtocolVersion, TASKS_EXTENSION_ID,
        UrlElicitationCapability,
    },
    service::{RequestContext, RoleClient},
};
use tokio::sync::watch;

use crate::handler_support::{Waiting, internal};
use crate::interaction::Interaction;

/// Protocol revisions this client can offer, newest first. Revision names are
/// ISO dates, so the lexicographic order is the chronological order.
pub(crate) fn supported_versions() -> Vec<ProtocolVersion> {
    let mut versions = ProtocolVersion::KNOWN_VERSIONS.to_vec();
    versions.sort_unstable_by(|a, b| b.as_str().cmp(a.as_str()));
    versions
}

#[derive(Clone)]
pub(crate) struct Handler {
    pub(crate) server: String,
    pub(crate) interaction: Arc<dyn Interaction>,
    pub(crate) call_key: Arc<Mutex<Option<String>>>,
    pub(crate) waiting: watch::Sender<bool>,
    /// The revision offered in `initialize`. `connect` retries the handshake
    /// with older revisions when a server mishandles a newer one.
    pub(crate) protocol_version: ProtocolVersion,
}

impl Handler {
    pub(crate) fn new(server: &str, interaction: Arc<dyn Interaction>) -> Self {
        Self {
            server: server.to_owned(),
            interaction,
            call_key: Arc::default(),
            waiting: watch::channel(false).0,
            protocol_version: supported_versions().into_iter().next().unwrap_or_default(),
        }
    }

    pub(crate) fn set_call_key(&self, key: Option<String>) {
        *self.call_key.lock().expect("MCP call key poisoned") = key;
    }

    pub(crate) fn waiting(&self) -> watch::Receiver<bool> {
        self.waiting.subscribe()
    }
}

impl ClientHandler for Handler {
    fn get_info(&self) -> ClientConfig {
        let mut info = ClientConfig::default();
        info.protocol_version = self.protocol_version.clone();
        info.capabilities.elicitation = Some(
            ElicitationCapability::new()
                .with_form(FormElicitationCapability::new())
                .with_url(UrlElicitationCapability::new()),
        );
        info.capabilities
            .extensions
            .get_or_insert_with(Default::default)
            .insert(TASKS_EXTENSION_ID.to_owned(), JsonObject::new());
        info
    }

    async fn create_elicitation(
        &self,
        request: ElicitRequestParams,
        context: RequestContext<RoleClient>,
    ) -> Result<ElicitResult, rmcp::ErrorData> {
        let interaction = self.request_context(request, &context)?;
        let _waiting = Waiting::new(&self.waiting);
        self.interaction
            .elicit(interaction)
            .await
            .map_err(|error| internal(format!("{}: {}", error.code, error.message)))
    }
}
