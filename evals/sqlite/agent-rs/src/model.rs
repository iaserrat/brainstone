//! A fault before the next inference proves the prior tool exchange was checkpointed.
use crate::{
    effects,
    protocol::{Error, Input},
};
use agent_rs::{
    Agent, HistoryEntry, Limits, Model, ModelConfig, ModelRequest, ModelStream, openai::OpenAiModel,
};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub struct FaultModel {
    pub inner: OpenAiModel,
    pub fault: Option<String>,
}

#[async_trait]
impl Model for FaultModel {
    fn binding(&self) -> Value {
        self.inner.binding()
    }

    async fn stream(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> agent_rs::Result<ModelStream> {
        let tool_completed = matches!(request.history.last(), Some(HistoryEntry::Exchange { results, .. }) if !results.is_empty());
        if tool_completed {
            crate::protocol::crash(self.fault.as_deref(), "after_tool_result");
        }
        self.inner.stream(request, cancel).await
    }
}

pub fn agent(input: &Input) -> Result<Agent<String, String>, Error> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let inner = agent_rs::openai::OpenAiModel::with_client(client, "eval-proxy", &input.endpoint)?;
    let model = Arc::new(FaultModel {
        inner,
        fault: input.fault.clone(),
    });
    let limits = Limits::default()
        .with_max_model_calls(4)
        .with_max_tokens(16_384);
    Ok(Agent::builder("sqlite-eval-v1", model)
        .config(ModelConfig::new(&input.model).with_temperature(0.0))
        .instructions(&input.system)
        .limits(limits)
        .tool(effects::tool(
            input.effects_database.clone(),
            input.fault.clone(),
        ))
        .input::<String>()
        .output::<String>()
        .build()?)
}
