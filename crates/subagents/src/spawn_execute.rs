use agent::{Model, ToolContext, ToolRequest};
use runtime::{Store, SubagentRequest, SubagentStore};
use serde_json::json;

use crate::{AgentTool, Error};

impl<S: Store + SubagentStore + 'static, M: Model + 'static> AgentTool<S, M> {
    pub(crate) async fn spawn(&self, request: ToolRequest) -> Result<String, Error> {
        let prepared = self.prepare(request)?;
        let spawned = self
            .0
            .runtime
            .spawn_subagent(&prepared.request)
            .await
            .map_err(Error::Runtime)?;
        if prepared.background {
            return Ok(json!({"agent_id":spawned.agent.id.get(),"status":"running"}).to_string());
        }
        self.wait(spawned.agent).await
    }

    fn prepare(&self, request: ToolRequest) -> Result<Prepared, Error> {
        let args: crate::spawn_args::SpawnArgs =
            serde_json::from_value(request.args).map_err(|error| Error::InvalidArguments {
                message: error.to_string(),
            })?;
        let context = request.context.ok_or(Error::DurableContextRequired)?;
        let config = self.config(&args, &context)?;
        let parent = operation_id(context.operation_id)?;
        let key = request
            .idempotency_key
            .ok_or(Error::DurableContextRequired)?;
        let request =
            SubagentRequest::new(parent, &key, &args.prompt, config).map_err(|error| {
                Error::InvalidArguments {
                    message: error.to_string(),
                }
            })?;
        Ok(Prepared {
            request,
            background: self.background(&args)?,
        })
    }

    fn config(
        &self,
        args: &crate::spawn_args::SpawnArgs,
        context: &ToolContext,
    ) -> Result<runtime::SubagentConfig, Error> {
        crate::spawn_authorize::check(context, &args.subagent_type, self.0.max_depth)?;
        let definition = self.definition(&args.subagent_type)?;
        let selected = self
            .0
            .registry()
            .select(definition.tools(), definition.disallowed_tools());
        crate::spawn_config::build(
            definition,
            context,
            args.model.as_deref(),
            selected.definitions(),
            self.background(args)?,
        )
    }

    fn definition(&self, name: &str) -> Result<&crate::Definition, Error> {
        self.0.catalog.get(name).ok_or_else(|| Error::UnknownAgent {
            name: name.to_owned(),
        })
    }

    fn background(&self, args: &crate::spawn_args::SpawnArgs) -> Result<bool, Error> {
        if !self.0.allow_background {
            return Ok(false);
        }
        Ok(
            self.definition(&args.subagent_type)?.background() == Some(true)
                || args.run_in_background.unwrap_or(true),
        )
    }
}

struct Prepared {
    request: SubagentRequest,
    background: bool,
}

fn operation_id(value: u64) -> Result<runtime::OperationId, Error> {
    let value = i64::try_from(value).map_err(|_| Error::DurableContextRequired)?;
    runtime::OperationId::new(value).map_err(|_| Error::DurableContextRequired)
}
