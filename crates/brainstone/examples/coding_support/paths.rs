use super::error::Error;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

pub(crate) fn visible(name: &str) -> bool {
    !name.starts_with('.') && name != "target"
}

pub(super) fn resolve(root: &Path, name: &str) -> Result<PathBuf, Error> {
    if name == "." {
        return Ok(root.to_owned());
    }
    if name.is_empty() {
        return Err(Error::InvalidPath);
    }
    validate(name)?;
    let mut path = root.to_owned();
    for component in Path::new(name).components() {
        path.push(component);
        if fs::symlink_metadata(&path)?.is_symlink() {
            return Err(Error::InvalidPath);
        }
    }
    Ok(path)
}

fn validate(name: &str) -> Result<(), Error> {
    for component in Path::new(name).components() {
        let Component::Normal(part) = component else {
            return Err(Error::InvalidPath);
        };
        let name = part.to_str().ok_or(Error::InvalidPath)?;
        if !visible(name) {
            return Err(Error::InvalidPath);
        }
    }
    Ok(())
}
