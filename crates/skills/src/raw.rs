use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawSkill {
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) description: Option<String>,
}
