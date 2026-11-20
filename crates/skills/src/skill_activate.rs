use serde::Deserialize;

use crate::{Error, skill_tool::SkillTool};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    name: String,
}

impl SkillTool {
    pub(crate) fn activate(&self, value: serde_json::Value) -> Result<String, Error> {
        let args: Args =
            serde_json::from_value(value).map_err(|error| Error::InvalidArguments {
                message: error.to_string(),
            })?;
        let skill = self
            .0
            .get(&args.name)
            .ok_or(Error::UnknownSkill { name: args.name })?;
        let (resources, truncated) =
            crate::resources::list(skill.directory()).map_err(|error| Error::ResourceIo {
                path: skill.directory().to_owned(),
                message: error.to_string(),
            })?;
        Ok(crate::skill_content::with_resources(
            skill, &resources, truncated,
        ))
    }
}
