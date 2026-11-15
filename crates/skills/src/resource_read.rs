use std::path::{Component, Path, PathBuf};
use std::{fs, str};

use serde::Deserialize;
use serde_json::json;

use crate::{Error, resource_tool::ResourceTool};

const MAX_RESOURCE_BYTES: usize = 512 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    name: String,
    path: String,
}

impl ResourceTool {
    pub(crate) fn read(&self, value: serde_json::Value) -> Result<String, Error> {
        let args: Args =
            serde_json::from_value(value).map_err(|error| Error::InvalidArguments {
                message: error.to_string(),
            })?;
        let skill = self.0.get(&args.name).ok_or_else(|| Error::UnknownSkill {
            name: args.name.clone(),
        })?;
        let path = resolve(skill.directory(), &args.path)?;
        let bytes = fs::read(&path).map_err(|error| Error::ResourceIo {
            path: path.clone(),
            message: error.to_string(),
        })?;
        if bytes.len() > MAX_RESOURCE_BYTES {
            return Err(Error::ResourceTooLarge { path });
        }
        let contents = str::from_utf8(&bytes).map_err(|_| Error::ResourceNotText { path })?;
        Ok(json!({"skill":args.name,"path":args.path,"contents":contents}).to_string())
    }
}

fn resolve(root: &Path, relative: &str) -> Result<PathBuf, Error> {
    let relative_path = Path::new(relative);
    let valid = !relative.is_empty()
        && relative != "SKILL.md"
        && relative_path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if !valid {
        return Err(Error::InvalidResourcePath {
            path: relative.to_owned(),
        });
    }
    canonical_resource(root, relative_path, relative)
}

fn canonical_resource(root: &Path, path: &Path, original: &str) -> Result<PathBuf, Error> {
    let root = root.canonicalize().map_err(|error| Error::ResourceIo {
        path: root.to_owned(),
        message: error.to_string(),
    })?;
    let candidate = root.join(path);
    let canonical = candidate
        .canonicalize()
        .map_err(|error| Error::ResourceIo {
            path: candidate,
            message: error.to_string(),
        })?;
    if !canonical.starts_with(&root) || !canonical.is_file() {
        return Err(Error::InvalidResourcePath {
            path: original.to_owned(),
        });
    }
    Ok(canonical)
}
