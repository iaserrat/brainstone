use std::{fs, path::Path};

use crate::{Catalog, Diagnostic, Skill};

const SKILL_FILE: &str = "SKILL.md";
const MAX_SKILL_DIRECTORIES: usize = 2000;

impl Catalog {
    #[must_use]
    /// Reads `skills/` under `root`; a missing directory yields an empty catalog.
    pub fn load(root: &Path) -> Self {
        if !root.exists() {
            return Self::default();
        }
        let mut catalog = Self::default();
        let mut paths = match directories(root) {
            Ok(paths) => paths,
            Err(error) => {
                catalog.diagnostics.push(error);
                return catalog;
            }
        };
        paths.sort();
        for directory in paths {
            catalog.load_one(&directory.join(SKILL_FILE));
        }
        catalog
    }

    fn load_one(&mut self, path: &Path) {
        if !path.exists() {
            return;
        }
        match load_skill(path) {
            Ok((skill, defaulted)) => {
                self.warn(&skill, defaulted);
                self.insert(skill);
            }
            Err(error) => self.diagnostics.push(error),
        }
    }

    fn warn(&mut self, skill: &Skill, defaulted: bool) {
        if defaulted {
            self.diagnostics
                .push(Diagnostic::defaulted(&skill.source, &skill.name));
        }
        let directory = skill
            .source
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str());
        if directory.is_some_and(|directory| directory != skill.name) {
            self.diagnostics.push(Diagnostic::mismatch(
                &skill.source,
                directory.expect("checked Some"),
                &skill.name,
            ));
        }
    }

    fn insert(&mut self, skill: Skill) {
        if self.skills.contains_key(&skill.name) {
            self.diagnostics.push(Diagnostic::invalid(
                &skill.source,
                format!("duplicate skill name {}", skill.name),
            ));
            return;
        }
        self.skills.insert(skill.name.clone(), skill);
    }
}

fn load_skill(path: &Path) -> Result<(Skill, bool), Diagnostic> {
    let text = fs::read_to_string(path).map_err(|error| Diagnostic::io(path, &error))?;
    let raw = crate::frontmatter::parse(path, &text)?;
    let defaulted = raw.name.as_deref().is_none_or(str::is_empty);
    Skill::build(path, &raw, text).map(|skill| (skill, defaulted))
}

fn directories(root: &Path) -> Result<Vec<std::path::PathBuf>, Diagnostic> {
    let entries = fs::read_dir(root).map_err(|error| Diagnostic::io(root, &error))?;
    let mut directories = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| Diagnostic::io(root, &error))?;
        if !entry.path().is_dir() {
            continue;
        }
        if directories.len() == MAX_SKILL_DIRECTORIES {
            return Err(Diagnostic::scan_limit(root, MAX_SKILL_DIRECTORIES));
        }
        directories.push(entry.path());
    }
    Ok(directories)
}
