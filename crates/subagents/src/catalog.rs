use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{Definition, Error};

/// The child agent definitions loaded from `agents/`, keyed by name.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    definitions: BTreeMap<String, Definition>,
}

impl Catalog {
    /// Load every Markdown definition below `root` in deterministic path order.
    ///
    /// # Errors
    ///
    /// Returns a stable definition error for unreadable or invalid files.
    pub fn load(root: &Path) -> Result<Self, Error> {
        if !root.exists() {
            return Ok(Self::default());
        }
        let mut definitions = BTreeMap::new();
        for path in markdown_files(root)? {
            let text = fs::read_to_string(&path).map_err(|error| io(&path, &error))?;
            let (raw, body) = crate::frontmatter::parse(&path, &text)?;
            let definition = Definition::build(&path, raw, body)?;
            let name = definition.name().to_owned();
            if definitions.insert(name.clone(), definition).is_some() {
                return Err(Error::DuplicateName { name });
            }
        }
        Ok(Self { definitions })
    }

    /// The definition loaded under `name`, if any.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Definition> {
        self.definitions.get(name)
    }

    /// Every definition, ordered by name.
    pub fn iter(&self) -> impl Iterator<Item = &Definition> {
        self.definitions.values()
    }

    /// Whether no definition loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

fn markdown_files(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut pending = vec![root.to_owned()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory).map_err(|error| io(&directory, &error))?;
        for entry in entries {
            let entry = entry.map_err(|error| io(&directory, &error))?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn io(path: &Path, error: &std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}
