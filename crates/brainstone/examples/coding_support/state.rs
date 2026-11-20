//! An effect journal belongs to exactly one canonical workspace location.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

pub(crate) fn database(root: &Path) -> Result<String, super::Error> {
    let directory = root.join(".brainstone");
    if directory.exists() && fs::symlink_metadata(&directory)?.file_type().is_symlink() {
        return Err("The state directory must not be a symlink".into());
    }
    fs::create_dir_all(&directory)?;
    let filename = directory.join("state.db");
    if fs::symlink_metadata(&filename).is_ok() && !directory.join("workspace-root").exists() {
        return Err("Existing state has no workspace identity; refusing to adopt it".into());
    }
    bind(root, &directory)?;
    if fs::symlink_metadata(&filename).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err("The state database must not be a symlink".into());
    }
    let filename = filename
        .to_str()
        .ok_or("Workspace path must be UTF-8")?
        .replace('%', "%25")
        .replace('?', "%3F")
        .replace('#', "%23");
    Ok(format!("sqlite://{filename}"))
}
fn bind(root: &Path, directory: &Path) -> Result<(), super::Error> {
    let identity = directory.join("workspace-root");
    let expected = root.to_str().ok_or("Workspace path must be UTF-8")?;
    if identity.exists() {
        if fs::symlink_metadata(&identity)?.file_type().is_symlink()
            || fs::read_to_string(&identity)? != expected
        {
            return Err(
                "State belongs to another workspace; copied databases must not redirect effects"
                    .into(),
            );
        }
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&identity)?;
    file.write_all(expected.as_bytes())?;
    file.sync_all()?;
    fs::File::open(directory)?.sync_all()?;
    Ok(())
}
