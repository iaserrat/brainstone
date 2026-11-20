use super::super::Runtime;
use crate::drive::Effects;
use crate::{Claimed, Error, Operation, OperationId, OperationState, Store};
use agent::{
    Model, Registry,
    observation::{self, Kind},
};

impl<S: Store> Runtime<S> {
    pub(super) async fn run_claim<M: Model>(
        &self,
        claimed: Claimed,
        model: &M,
        tools: &Registry,
    ) -> Result<Option<Operation>, Error> {
        let span = observation::span(Kind::Operation);
        span.record("operation_id", claimed.operation.id.get());
        span.record("session_id", claimed.operation.session_id.get());
        span.record("lane_id", claimed.operation.lane_id.get());
        span.record("attempt", claimed.lease.token);
        observation::run(
            span,
            Box::pin(self.run_claim_inner(claimed, model, tools)),
            outcome,
        )
        .await
    }

    async fn run_claim_inner<M: Model>(
        &self,
        mut claimed: Claimed,
        model: &M,
        tools: &Registry,
    ) -> Result<Option<Operation>, Error> {
        let id = claimed.operation.id;
        let effects = Effects {
            model,
            tools,
            compaction: self.config.compaction.as_ref(),
            policy: self.config.tool_policy.as_deref(),
        };
        let result =
            crate::drive::drive(&self.store, &mut claimed, self.config.lease_ttl, effects).await;
        if let Err(error) = result
            && !(error == Error::LeaseLost && self.terminal(id).await?)
        {
            return Err(error);
        }
        self.operation(id)
            .await?
            .map_or_else(missing, |item| Ok(Some(item)))
    }

    async fn terminal(&self, id: OperationId) -> Result<bool, Error> {
        Ok(self.operation(id).await?.is_some_and(|operation| {
            matches!(
                operation.state,
                OperationState::Completed | OperationState::Failed
            )
        }))
    }
}

fn missing() -> Result<Option<Operation>, Error> {
    Err(Error::CorruptRecord {
        message: "claimed operation disappeared".to_owned(),
    })
}

fn outcome(result: &Result<Option<Operation>, Error>) -> &'static str {
    match result {
        Ok(Some(operation)) if operation.state == OperationState::Completed => "ok",
        Ok(Some(operation)) if matches!(&operation.outcome, Some(agent::Outcome::Failed { code, .. }) if code == "agent_cancelled") => {
            "cancelled"
        }
        Err(Error::LeaseLost) => "lease_lost",
        _ => "failed",
    }
}
