use std::path::{Path, PathBuf};

use crate::{Diagnostic, raw::RawSkill};

const MAX_NAME_CHARS: usize = 64;
const MAX_DESCRIPTION_CHARS: usize = 1024;

/// One loaded skill: its name, description, source path, and instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) source: PathBuf,
    pub(crate) contents: String,
}

impl Skill {
    pub(crate) fn build(path: &Path, raw: &RawSkill, contents: String) -> Result<Self, Diagnostic> {
        let name = clean(raw.name.as_deref().unwrap_or_else(|| directory_name(path)));
        let description = clean(raw.description.as_deref().unwrap_or_default());
        validate(path, &name, &description)?;
        Ok(Self {
            name,
            description,
            source: path.to_owned(),
            contents,
        })
    }

    #[must_use]
    /// The skill's name, unique within the catalog.
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    /// The one-line description the index shows the model.
    pub fn description(&self) -> &str {
        &self.description
    }

    pub(crate) fn directory(&self) -> &Path {
        self.source
            .parent()
            .expect("SKILL.md has a parent directory")
    }
}

fn directory_name(path: &Path) -> &str {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

fn clean(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate(path: &Path, name: &str, description: &str) -> Result<(), Diagnostic> {
    let valid_name = !name.is_empty()
        && name.chars().count() <= MAX_NAME_CHARS
        && name
            .chars()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || value == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--");
    if !valid_name {
        return Err(Diagnostic::invalid(path, "invalid name"));
    }
    if description.is_empty() || description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(Diagnostic::invalid(path, "invalid description"));
    }
    Ok(())
}
