use crate::{ConfigError, settings_parse::required, target_names::Names};
use reqwest::header::HeaderMap;
use std::{collections::HashMap, num::NonZeroUsize, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Protocol {
    Responses,
    ChatCompletions,
}

#[derive(Clone)]
pub(crate) struct TargetOptions {
    pub(crate) endpoint: String,
    pub(crate) target: String,
    pub(crate) protocol: Protocol,
    pub(crate) capabilities: agent::ModelCapabilities,
    pub(crate) parameters: openai::RequestParameters,
    pub(crate) headers: HeaderMap,
    pub(crate) idle: Duration,
    pub(crate) context: Option<NonZeroUsize>,
    pub(crate) effort: Option<openai::Effort>,
    pub(crate) output: Option<u32>,
    pub(crate) reasoning_summary: bool,
}

impl TargetOptions {
    pub(crate) fn parse(
        values: &HashMap<String, String>,
        names: &Names,
    ) -> Result<Option<Self>, ConfigError> {
        if !values.contains_key(names.endpoint) {
            return crate::target_parse::orphaned(values, names);
        }
        crate::target_parse::validate(values, names)?;
        let protocol = crate::target_parse::protocol(values, names)?;
        Ok(Some(Self {
            endpoint: required(values, names.endpoint)?,
            target: crate::target_validation::identity(values, names)?,
            protocol,
            capabilities: crate::target_capabilities::parse(values, names.capabilities)?,
            parameters: crate::target_payload::parameters(values, names.parameters)?,
            headers: crate::target_payload::headers(values, names.headers)?,
            idle: crate::target_limits::idle(values, names.idle)?,
            context: crate::target_limits::context(values, names.context)?,
            effort: crate::target_parse::effort(values, names.effort)?,
            output: crate::target_parse::output(values, names)?,
            reasoning_summary: reasoning_summary(values, names.reasoning_summary, protocol)?,
        }))
    }
}

/// Only the Responses request has a summary field to turn on.
fn reasoning_summary(
    values: &HashMap<String, String>,
    name: &'static str,
    protocol: Protocol,
) -> Result<bool, ConfigError> {
    let enabled = crate::settings_parse::flag(values, name)?;
    if enabled && protocol != Protocol::Responses {
        return Err(ConfigError::Invalid { name });
    }
    Ok(enabled)
}
