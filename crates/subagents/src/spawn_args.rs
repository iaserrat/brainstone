use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct SpawnArgs {
    pub(crate) subagent_type: String,
    pub(crate) prompt: String,
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) run_in_background: Option<bool>,
}
