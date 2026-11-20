use agent::{ExecutionProfile, ToolContext};
use runtime::SubagentConfig;

use crate::{Definition, Effort, Error, Model};

pub(crate) fn build(
    definition: &Definition,
    context: &ToolContext,
    invocation_model: Option<&str>,
    definitions: Vec<serde_json::Value>,
    background: bool,
) -> Result<SubagentConfig, Error> {
    let depth = context
        .profile
        .depth()
        .checked_add(1)
        .ok_or(Error::DepthExceeded)?;
    let model = resolve_model(invocation_model, definition.model(), context);
    let effort = definition
        .effort()
        .map(map_effort)
        .or(context.profile.effort());
    let max_turns = max_turns(definition, context);
    let profile = ExecutionProfile::new(model.as_deref(), effort, max_turns, depth)
        .map_err(|error| Error::InvalidProfile {
            message: error.to_string(),
        })?
        .with_allowed_agents(allowed_agents(definition.tools()));
    Ok(
        SubagentConfig::new(definition.name(), definition.system_prompt(), profile)
            .map_err(|error| Error::InvalidProfile {
                message: error.to_string(),
            })?
            .with_tools(definitions)
            .in_background(background),
    )
}

fn max_turns(definition: &Definition, context: &ToolContext) -> Option<usize> {
    definition
        .max_turns()
        .map_or_else(|| context.profile.max_turns(), agent::TurnLimit::get)
}

fn resolve_model(invocation: Option<&str>, model: &Model, context: &ToolContext) -> Option<String> {
    invocation.map(resolve_alias).or_else(|| match model {
        Model::Inherit => context.profile.model().map(str::to_owned),
        Model::Alias(value) | Model::Id(value) => Some(resolve_alias(value)),
    })
}

fn resolve_alias(model: &str) -> String {
    match model {
        "sonnet" => "claude-sonnet-5",
        "opus" => "claude-opus-5",
        "haiku" => "claude-haiku-4-5",
        "fable" => "claude-fable-5",
        other => other,
    }
    .to_owned()
}

fn map_effort(effort: Effort) -> agent::Effort {
    match effort {
        Effort::Low => agent::Effort::Low,
        Effort::Medium => agent::Effort::Medium,
        Effort::High => agent::Effort::High,
        Effort::XHigh => agent::Effort::XHigh,
        Effort::Max => agent::Effort::Max,
    }
}

fn allowed_agents(tools: Option<&[String]>) -> Option<Vec<String>> {
    let tools = tools?;
    let Some(agent) = tools
        .iter()
        .find(|tool| tool.as_str() == "Agent" || tool.starts_with("Agent("))
    else {
        return Some(Vec::new());
    };
    let values = agent
        .strip_prefix("Agent(")
        .and_then(|value| value.strip_suffix(')'))?;
    Some(
        values
            .split(',')
            .map(str::trim)
            .map(str::to_owned)
            .collect(),
    )
}
