use std::{fs::OpenOptions, path::Path};
use crate::Error;

/// `SQLite` inherits the database file's permissions for its WAL sidecars.
pub(crate) fn create(path: &Path) -> Result<(), Error> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(crate::sqlite_error::store(sqlx::Error::Io(error))),
    }
}
