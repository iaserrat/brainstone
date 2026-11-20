use rmcp::{
    model::{ElicitRequestParams, ElicitResult},
    service::{RequestContext, RoleClient},
};

use crate::handler::Handler;
use crate::handler_support::{Waiting, internal, no_call_key};
use crate::interaction::InteractionContext;

impl Handler {
    pub(crate) async fn elicit(
        &self,
        request: ElicitRequestParams,
        request_id: &str,
    ) -> Result<ElicitResult, rmcp::ErrorData> {
        let interaction = self.direct_context(request, request_id)?;
        let _waiting = Waiting::new(&self.waiting);
        self.interaction
            .elicit(interaction)
            .await
            .map_err(|error| internal(format!("{}: {}", error.code, error.message)))
    }

    pub(crate) fn request_context(
        &self,
        request: ElicitRequestParams,
        context: &RequestContext<RoleClient>,
    ) -> Result<InteractionContext, rmcp::ErrorData> {
        let request_id = serde_json::to_string(&context.id).map_err(internal)?;
        self.direct_context(request, &request_id)
    }

    fn direct_context(
        &self,
        request: ElicitRequestParams,
        request_id: &str,
    ) -> Result<InteractionContext, rmcp::ErrorData> {
        let call_key = self
            .call_key
            .lock()
            .expect("MCP call key poisoned")
            .clone()
            .ok_or_else(no_call_key)?;
        Ok(InteractionContext {
            server: self.server.clone(),
            call_key,
            request_id: request_id.to_owned(),
            request,
        })
    }
}
