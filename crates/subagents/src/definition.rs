use std::path::{Path, PathBuf};

use crate::{Effort, Error, Model, raw::RawDefinition};

/// One child agent's definition: its prompt, tools, model, and limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) system_prompt: String,
    pub(crate) tools: Option<Vec<String>>,
    pub(crate) disallowed_tools: Vec<String>,
    pub(crate) model: Model,
    pub(crate) max_turns: Option<agent::TurnLimit>,
    pub(crate) effort: Option<Effort>,
    pub(crate) background: Option<bool>,
    pub(crate) mcp_servers: Vec<String>,
    pub(crate) source: PathBuf,
}

impl Definition {
    pub(crate) fn build(path: &Path, raw: RawDefinition, body: String) -> Result<Self, Error> {
        validate(path, &raw, &body)?;
        let mcp_servers = raw
            .mcp_servers
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect();
        Ok(Self {
            name: raw.name,
            description: raw.description,
            system_prompt: body,
            tools: raw.tools.map(crate::raw::StringList::items),
            disallowed_tools: raw
                .disallowed_tools
                .map_or_else(Vec::new, crate::raw::StringList::items),
            model: Model::parse(raw.model),
            max_turns: turn_limit(path, raw.max_turns)?,
            effort: raw.effort,
            background: raw.background,
            mcp_servers,
            source: path.to_owned(),
        })
    }
}

fn validate(path: &Path, raw: &RawDefinition, body: &str) -> Result<(), Error> {
    if let Some(field) = raw.unsupported() {
        return Err(Error::UnsupportedField {
            path: path.to_owned(),
            field,
        });
    }
    if raw.mcp_servers.iter().any(|value| !value.is_string()) {
        return Err(Error::UnsupportedField {
            path: path.to_owned(),
            field: "mcpServers",
        });
    }
    require(
        path,
        "name",
        !raw.name.is_empty() && raw.name.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
    )?;
    require(path, "description", !raw.description.trim().is_empty())?;
    require(path, "system prompt", !body.trim().is_empty())?;
    Ok(())
}

fn require(path: &Path, field: &'static str, valid: bool) -> Result<(), Error> {
    valid.then_some(()).ok_or_else(|| Error::InvalidDefinition {
        path: path.to_owned(),
        field,
        message: "value is invalid or empty".to_owned(),
    })
}

fn turn_limit(
    path: &Path,
    value: Option<serde_json::Value>,
) -> Result<Option<agent::TurnLimit>, Error> {
    // Preserve the established definition error for zero, rather than a YAML error.
    require(
        path,
        "maxTurns",
        value.as_ref().is_none_or(|value| value.as_u64() != Some(0)),
    )?;
    value
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| Error::InvalidFrontmatter {
            path: path.to_owned(),
            message: error.to_string(),
        })
}
