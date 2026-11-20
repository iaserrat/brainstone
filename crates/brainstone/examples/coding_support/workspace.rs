//! Bounded access to a trusted workspace, not an operating-system sandbox.
use brainstone::ToolError;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

#[path = "error.rs"]
pub(crate) mod error;
#[path = "paths.rs"]
mod paths;
use error::Error;

pub(crate) const MAX_FILE_BYTES: usize = 64 * 1024;

/// Clones share the canonical root captured by the three tool closures.
#[derive(Debug, Clone)]
pub(crate) struct Workspace(Arc<PathBuf>);

impl Workspace {
    pub(crate) fn open(root: &Path) -> Result<Self, ToolError> {
        let metadata = fs::symlink_metadata(root).map_err(Error::from)?;
        if metadata.is_symlink() || !metadata.is_dir() {
            return Err(Error::InvalidPath.into());
        }
        Ok(Self(Arc::new(root.canonicalize().map_err(Error::from)?)))
    }

    pub(crate) fn root(&self) -> &Path {
        &self.0
    }

    pub(crate) fn resolve(&self, name: &str) -> Result<PathBuf, ToolError> {
        paths::resolve(self.root(), name).map_err(Into::into)
    }

    pub(crate) fn text(&self, name: &str) -> Result<(PathBuf, String), ToolError> {
        let path = self.resolve(name)?;
        if !fs::metadata(&path).map_err(Error::from)?.is_file() {
            return Err(Error::InvalidPath.into());
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .map_err(Error::from)?
            .take((MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(Error::from)?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(Error::FileTooLarge.into());
        }
        let text = String::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?;
        Ok((path, text))
    }
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
