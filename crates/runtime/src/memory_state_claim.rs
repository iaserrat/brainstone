use std::time::Duration;

use crate::memory_state::State;
use crate::{Claimed, OperationId};

impl State {
    pub(crate) fn claim(&mut self, worker: &str, ttl: Duration) -> Option<Claimed> {
        self.release_due();
        let mut claimed = self.runs.claim(&mut self.operations, worker, ttl)?;
        claimed.tool_grant = self.tool_grant(claimed.operation.session_id);
        Some(claimed)
    }

    pub(crate) fn claim_operation(
        &mut self,
        id: OperationId,
        worker: &str,
        ttl: Duration,
    ) -> Option<Claimed> {
        let mut claimed = self
            .runs
            .claim_operation(&mut self.operations, id, worker, ttl)?;
        claimed.tool_grant = self.tool_grant(claimed.operation.session_id);
        Some(claimed)
    }

    fn tool_grant(&self, session: crate::SessionId) -> Option<Vec<String>> {
        self.subagents
            .values()
            .find(|(_, agent, _)| agent.operation.session_id == session)
            .map(|(request, _, _)| request.config.tool_grant())
    }
}
