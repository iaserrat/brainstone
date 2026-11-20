use crate::configured::ConfigError;
use serde::Deserialize;
use std::path::Path;
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Rules {
    #[serde(default)]
    require_approval: Vec<String>,
    #[serde(default)]
    deny: Vec<String>,
}
pub(crate) fn load(
    root: &Path,
    mcp: &mcp::Config,
) -> Result<Option<Box<runtime::ToolPolicy>>, ConfigError> {
    let source = match std::fs::read_to_string(root.join("tool-policy.toml")) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err(ConfigError::Unreadable {
                name: "tool-policy.toml",
            });
        }
    };
    let rules: Rules = toml::from_str(&source).map_err(|_| ConfigError::Invalid {
        name: "tool-policy.toml",
    })?;
    runtime::ToolPolicy::new(
        rules.require_approval,
        rules.deny,
        format!("{}\n{source}", mcp.execution_binding()),
    )
    .map(|policy| Some(Box::new(policy)))
    .map_err(|_| ConfigError::Invalid {
        name: "tool-policy.toml",
    })
}
