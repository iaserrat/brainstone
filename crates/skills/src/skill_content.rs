use crate::Skill;

pub(crate) fn explicit(skill: &Skill) -> String {
    format!(
        "<skill_content name=\"{}\">\n{}\n\nSkill directory: {}\nRelative resource paths are resolved from this directory with SkillResource.\n</skill_content>",
        skill.name,
        skill.contents,
        skill.directory().display()
    )
}

pub(crate) fn with_resources(skill: &Skill, resources: &[String], truncated: bool) -> String {
    let resource_lines = resources
        .iter()
        .map(|path| format!("<file>{}</file>", escape_xml(path)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<skill_content name=\"{}\">\n{}\n\nSkill directory: {}\nRelative resource paths are resolved from this directory.\n<skill_resources truncated=\"{truncated}\">\n{resource_lines}\n</skill_resources>\n</skill_content>",
        skill.name,
        skill.contents,
        skill.directory().display()
    )
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
