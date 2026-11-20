use std::sync::Arc;

use brainstone::openai::ChatCompletionsModel;
use brainstone::runtime::SqliteStore;
use brainstone::{Host, Registry, Runtime};
use sqlx::SqlitePool;

use super::{model::EvalModel, protocol::Input, tool::RecordValue};

type Error = Box<dyn std::error::Error>;

pub(crate) fn host(
    runtime: Runtime<SqliteStore>,
    input: &Input,
    pool: SqlitePool,
) -> Result<Host<SqliteStore, EvalModel>, Error> {
    let tools = Registry::new(vec![Arc::new(RecordValue {
        pool,
        crash_after_effect: input.fault.as_deref() == Some("after_effect"),
    })]);
    let inner = ChatCompletionsModel::new(
        &format!("{}/chat/completions", input.endpoint),
        &input.model,
        "eval-proxy",
    )?
    .with_max_output_tokens(768);
    let model = EvalModel {
        inner,
        crash_after_tool_result: input.fault.as_deref() == Some("after_tool_result"),
    };
    Ok(Host::new(runtime, model, tools))
}

pub(crate) fn profile(input: &Input) -> Result<agent::ExecutionProfile, Error> {
    let mut profile = agent::ExecutionProfile::new(Some(&input.model), None, 4, 0)?;
    if let Some(schema) = &input.output_schema {
        profile = profile.with_output_schema(brainstone::OutputSchema::new(
            "eval_answer",
            schema.clone(),
        )?);
    }
    Ok(profile)
}
