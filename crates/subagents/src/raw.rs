use serde::Deserialize;
use serde_json::Value;

use crate::Effort;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum StringList {
    Text(String),
    List(Vec<String>),
}

impl StringList {
    pub(crate) fn items(self) -> Vec<String> {
        match self {
            Self::Text(value) => value.split(',').map(str::trim).map(str::to_owned).collect(),
            Self::List(values) => values,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RawDefinition {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) tools: Option<StringList>,
    #[serde(default)]
    pub(crate) disallowed_tools: Option<StringList>,
    pub(crate) model: Option<String>,
    pub(crate) max_turns: Option<Value>,
    pub(crate) effort: Option<Effort>,
    pub(crate) background: Option<bool>,
    #[serde(default)]
    pub(crate) mcp_servers: Vec<Value>,
    pub(crate) permission_mode: Option<serde::de::IgnoredAny>,
    pub(crate) skills: Option<serde::de::IgnoredAny>,
    pub(crate) hooks: Option<serde::de::IgnoredAny>,
    pub(crate) memory: Option<serde::de::IgnoredAny>,
    pub(crate) isolation: Option<serde::de::IgnoredAny>,
    pub(crate) color: Option<serde::de::IgnoredAny>,
    pub(crate) initial_prompt: Option<serde::de::IgnoredAny>,
}

impl RawDefinition {
    pub(crate) fn unsupported(&self) -> Option<&'static str> {
        [
            ("permissionMode", self.permission_mode.is_some()),
            ("skills", self.skills.is_some()),
            ("hooks", self.hooks.is_some()),
            ("memory", self.memory.is_some()),
            ("isolation", self.isolation.is_some()),
            ("color", self.color.is_some()),
            ("initialPrompt", self.initial_prompt.is_some()),
        ]
        .into_iter()
        .find_map(|(name, set)| set.then_some(name))
    }
}
